use std::{
    io::{BufReader, BufWriter, Read, Write},
    path::PathBuf,
};

use anyhow::{bail, ensure};
use dap::prelude::*;
use serde::Deserialize;
use y86_sim::{
    architectures::create_sim,
    framework::{CpuSim, MemData},
};

use crate::SimOption;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunProgKind {
    SingleStep,
    Run,
    InitialRun,
}

enum ServerStatus {
    /// The server is running the program so it does not accept any request.
    RunProg(RunProgKind),
    /// The program execution is stopped and the server is waiting for client
    /// request.
    ServeReq,
}

pub struct Inner {
    source_path: PathBuf,
    source_info: y86_sim::SourceInfo,
    source_name: String,
    scopes: Vec<types::Scope>,
    stage_info: Vec<y86_sim::framework::StageInfo>,
    sim: Box<dyn CpuSim>,
}

pub struct DebugServer<R: Read, W: Write> {
    inner: Option<Inner>,
    /// Lines of breakpoints
    breakpoints: Vec<types::Breakpoint>,
    /// Path of the source file (this debugger only supports single source file)
    server: dap::server::Server<R, W>,
    status: ServerStatus,
    sim_opt: SimOption,
}

const THREAD_ID: i64 = 1;
const STACK_FRAME_ID: i64 = 1;
const REG_SCOPE_VAR_REF: i64 = 1;

const VAR_PRESENTATION_HINT: types::VariablePresentationHint = types::VariablePresentationHint {
    kind: Some(types::VariablePresentationHintKind::Data),
    attributes: None,
    visibility: None,
    lazy: Some(false),
};

#[derive(Deserialize)]
struct LaunchOption {
    program: String,
}

impl<R: Read, W: Write> DebugServer<R, W> {
    pub fn new(input: R, output: W, option: SimOption) -> Self {
        Self {
            inner: None,
            server: dap::server::Server::new(BufReader::new(input), BufWriter::new(output)),
            breakpoints: Vec::new(),
            status: ServerStatus::ServeReq,
            sim_opt: option,
        }
    }

    fn inner(&self) -> anyhow::Result<&Inner> {
        self.inner
            .as_ref()
            .ok_or(anyhow::anyhow!("program data not initialized"))
    }

    fn init_program(&mut self, program: PathBuf) -> anyhow::Result<()> {
        tracing::info!("initializing program: {}", program.display());

        let src = std::fs::read_to_string(&program)?;
        let a = y86_sim::assemble(&src, y86_sim::AssembleOption::default())?;

        let mem = MemData::init(a.obj.init_mem());
        let sim = create_sim(self.sim_opt.arch.clone(), mem, false);
        let source_path = program.clone();
        let source_info = a.source;
        let source_name = program.file_name().unwrap().to_string_lossy().to_string();
        let main_source = types::Source {
            name: Some(source_name.clone()),
            path: Some(source_path.display().to_string()),
            presentation_hint: Some(types::PresentationHint::Normal),
            ..Default::default()
        };

        let mut scopes: Vec<_> = sim
            .get_stage_info()
            .into_iter()
            .enumerate()
            .map(|(index, s)| types::Scope {
                name: s.name.to_string(),
                presentation_hint: Some(types::ScopePresentationhint::Locals),
                variables_reference: (index + 2) as i64,
                expensive: false,
                source: Some(main_source.clone()),
                ..Default::default()
            })
            .collect();

        scopes.insert(
            0,
            types::Scope {
                name: "Registers".to_string(),
                presentation_hint: Some(types::ScopePresentationhint::Registers),
                variables_reference: REG_SCOPE_VAR_REF,
                expensive: false,
                source: Some(self.main_source()),
                ..Default::default()
            },
        );

        let stage_info = sim.get_stage_info();

        self.inner = Some(Inner {
            source_path,
            source_info,
            source_name,
            scopes,
            stage_info,
            sim,
        });

        Ok(())
    }

    fn main_source(&self) -> types::Source {
        types::Source {
            name: self.inner.as_ref().map(|i| i.source_name.clone()),
            path: self
                .inner
                .as_ref()
                .map(|p| p.source_path.display().to_string()),
            presentation_hint: Some(types::PresentationHint::Normal),
            ..Default::default()
        }
    }

    fn handle_request(&mut self, req: Request) -> anyhow::Result<(Response, ServerStatus)> {
        tracing::trace!(?req);

        match &req.command {
            Command::Launch(args) => {
                let Some(data) = &args.additional_data else {
                    bail!("missing additional data");
                };
                let options: LaunchOption = serde_json::from_value(data.clone())?;

                self.init_program(PathBuf::from(options.program))?;

                Ok((req.success(ResponseBody::Launch), ServerStatus::ServeReq))
            }
            Command::Disconnect(_) => {
                bail!("can not handle disconnect command in the handler");
            }
            Command::ConfigurationDone => Ok((
                req.success(ResponseBody::ConfigurationDone),
                ServerStatus::RunProg(RunProgKind::InitialRun),
            )),
            Command::SetBreakpoints(args) => {
                let Some(breakpoints) = &args.breakpoints else {
                    bail!("missing breakpoints");
                };

                let source_path = args
                    .source
                    .path
                    .clone()
                    .ok_or(anyhow::anyhow!("missing source name"))?;
                let source = self.main_source();
                if Some(source_path) != source.path {
                    bail!("source path mismatch");
                }

                let inner = self.inner()?;

                let bps: Vec<types::Breakpoint> = breakpoints
                    .iter()
                    .map(|b| {
                        let verified = true;
                        let ln = inner.source_info.get_line(b.line).unwrap();
                        let message = ln.addr.map(|a| format!("addr: {:#x}", a));

                        let Some(addr) = ln.addr else {
                            return types::Breakpoint::default();
                        };

                        types::Breakpoint {
                            // we use the address as the id
                            id: Some(addr as i64),
                            verified,
                            message,
                            source: Some(source.clone()),
                            line: Some(b.line),
                            ..Default::default()
                        }
                    })
                    .collect();

                self.breakpoints = bps.clone();

                Ok((
                    req.success(ResponseBody::SetBreakpoints(
                        responses::SetBreakpointsResponse { breakpoints: bps },
                    )),
                    ServerStatus::ServeReq,
                ))
            }
            Command::SetExceptionBreakpoints(args) => {
                // todo: add support for exception breakpoints (e.g. Stat::Adr)
                ensure!(args.filters.is_empty(), "filters not supported");
                ensure!(
                    args.filter_options.is_none() && args.exception_options.is_none(),
                    "filter_options and exception_options not supported"
                );
                Ok((
                    req.success(ResponseBody::SetExceptionBreakpoints(
                        responses::SetExceptionBreakpointsResponse { breakpoints: None },
                    )),
                    ServerStatus::ServeReq,
                ))
            }
            Command::Threads => Ok((
                req.success(ResponseBody::Threads(
                    // we have only one thread
                    responses::ThreadsResponse {
                        threads: vec![types::Thread {
                            id: THREAD_ID,
                            name: "main".to_string(),
                        }],
                    },
                )),
                ServerStatus::ServeReq,
            )),
            Command::StackTrace(args) => {
                if args.thread_id != THREAD_ID {
                    bail!("invalid thread id");
                }
                // not the first frame, we don't have stack trace
                if args.start_frame.unwrap_or_default() > 0 {
                    return Ok((
                        req.success(ResponseBody::StackTrace(responses::StackTraceResponse {
                            stack_frames: vec![],
                            total_frames: None,
                        })),
                        ServerStatus::ServeReq,
                    ));
                }
                let inner = self.inner()?;
                // currently we don't have stack trace, thus we return a single frame
                Ok((
                    req.success(ResponseBody::StackTrace(responses::StackTraceResponse {
                        stack_frames: vec![types::StackFrame {
                            id: STACK_FRAME_ID,
                            name: "current".to_string(),
                            source: Some(self.main_source()),
                            line: inner
                                .source_info
                                .get_line_number_by_addr(inner.sim.program_counter())
                                .unwrap_or_default(),
                            can_restart: Some(false),
                            presentation_hint: Some(types::StackFramePresentationhint::Normal),
                            ..Default::default()
                        }],
                        total_frames: None,
                    })),
                    ServerStatus::ServeReq,
                ))
            }
            Command::Scopes(args) => {
                if args.frame_id != STACK_FRAME_ID {
                    bail!("invalid frame id");
                }
                Ok((
                    req.success(ResponseBody::Scopes(responses::ScopesResponse {
                        scopes: self.inner()?.scopes.clone(),
                    })),
                    ServerStatus::ServeReq,
                ))
            }
            Command::Variables(args) => {
                let inner = self.inner()?;

                // register scope
                let vars = if args.variables_reference == REG_SCOPE_VAR_REF {
                    let regs = inner.sim.registers();
                    regs.iter()
                        .enumerate()
                        .map(|(reg, val)| {
                            let value = format!("{:#x}", val);
                            types::Variable {
                                name: y86_sim::isa::reg_code::name_of(reg as u8).to_string(),
                                value,
                                presentation_hint: Some(VAR_PRESENTATION_HINT),
                                ..Default::default()
                            }
                        })
                        .collect()
                } else {
                    // scopes of each stage
                    let index = args.variables_reference - 2;
                    if index < 0 {
                        bail!("invalid variables reference {index}");
                    }
                    let stage = inner
                        .stage_info
                        .get(index as usize)
                        .ok_or(anyhow::anyhow!("invalid stage index {index}"))?;
                    stage
                        .signals
                        .iter()
                        .map(|(name, val)| types::Variable {
                            name: name.to_string(),
                            value: val.clone(),
                            presentation_hint: Some(VAR_PRESENTATION_HINT),
                            ..Default::default()
                        })
                        .collect()
                };
                Ok((
                    req.success(ResponseBody::Variables(responses::VariablesResponse {
                        variables: vars,
                    })),
                    ServerStatus::ServeReq,
                ))
            }
            Command::Next(args) => {
                if args.thread_id != THREAD_ID {
                    bail!("invalid thread id");
                }
                // we do not care about the granularity
                Ok((
                    req.success(ResponseBody::Next),
                    ServerStatus::RunProg(RunProgKind::SingleStep),
                ))
            }
            Command::Continue(args) => {
                if args.thread_id != THREAD_ID {
                    bail!("invalid thread id");
                }
                Ok((
                    req.success(ResponseBody::Continue(responses::ContinueResponse {
                        // we have exactly one thread
                        all_threads_continued: Some(true),
                    })),
                    ServerStatus::RunProg(RunProgKind::Run),
                ))
            }
            _ => {
                bail!("ydb: not implemented");
            }
        }
    }

    fn init(&mut self) -> anyhow::Result<()> {
        tracing::trace!("waiting for init request");

        let req = match self.server.poll_request()? {
            Some(req) => req,
            None => bail!("no request"),
        };
        let Command::Initialize(_) = req.command else {
            bail!("expected initialize command to be the first request");
        };
        let rsp = req.success(ResponseBody::Initialize(types::Capabilities {
            supports_configuration_done_request: Some(true),
            ..Default::default()
        }));

        // When you call respond, send_event etc. the message will be wrapped
        // in a base message with a appropriate seq number, so you don't have to keep
        // track of that yourself
        self.server.respond(rsp)?;

        self.server.send_event(Event::Initialized)?;
        Ok(())
    }

    fn serve_req(&mut self) -> anyhow::Result<()> {
        let req = match self.server.poll_request()? {
            Some(req) => req,
            None => bail!("no request"),
        };

        if let Command::Disconnect(_) = req.command {
            bail!("disconnect command received");
        }

        let seq = req.seq;

        let rsp = match self.handle_request(req) {
            Ok((rsp, next_status)) => {
                self.status = next_status;
                rsp
            }
            Err(e) => {
                tracing::error!(?e);
                Response {
                    request_seq: seq,
                    success: false,
                    message: Some(responses::ResponseMessage::Error(format!("{:?}", e))),
                    body: None,
                    error: None,
                }
            }
        };
        self.server.respond(rsp)?;
        Ok(())
    }

    fn run_prog(&mut self, kind: RunProgKind) -> anyhow::Result<()> {
        if kind == RunProgKind::InitialRun {
            self.server
                .send_event(Event::Stopped(events::StoppedEventBody {
                    reason: types::StoppedEventReason::Entry,
                    description: Some("Stop at beginning".to_string()),
                    thread_id: Some(THREAD_ID),
                    preserve_focus_hint: Some(false),
                    text: None,
                    all_threads_stopped: None,
                    hit_breakpoint_ids: None,
                }))?;
            self.status = ServerStatus::ServeReq;
            return Ok(());
        }

        let inner = self
            .inner
            .as_mut()
            .ok_or(anyhow::anyhow!("program data not initialized"))?;
        let sim = &mut inner.sim;

        // start the simulation loop
        loop {
            if sim.is_terminate() {
                tracing::info!("program terminated");
                self.server
                    .send_event(Event::Stopped(events::StoppedEventBody {
                        reason: types::StoppedEventReason::Pause,
                        description: Some("Pause on termination".to_string()),
                        thread_id: Some(THREAD_ID),
                        preserve_focus_hint: None,
                        text: Some(format!(
                            "pc = {:#x}, cycle count = {}",
                            sim.program_counter(),
                            sim.cycle_count()
                        )),
                        all_threads_stopped: None,
                        hit_breakpoint_ids: None,
                    }))?;
                self.status = ServerStatus::ServeReq;
                break;
            }

            tracing::trace!("cycle count: {}", sim.cycle_count());
            sim.initiate_next_cycle();
            sim.propagate_signals();

            let pc = sim.program_counter();

            if let Some(bp) = self.breakpoints.iter().find(|bp| {
                let Some(bp_ln) = bp.line else { return false };
                inner
                    .source_info
                    .get_line_number_by_addr(pc)
                    .map(|ln| ln == bp_ln)
                    .unwrap_or(false)
            }) {
                let bp_id = bp.id.ok_or(anyhow::anyhow!("breakpoint id not set"))?;
                tracing::trace!("hit breakpoint: line = {:?}", bp.line);
                self.server
                    .send_event(Event::Stopped(events::StoppedEventBody {
                        reason: types::StoppedEventReason::Breakpoint,
                        description: Some("Stop at breakpoint".to_string()),
                        thread_id: Some(THREAD_ID),
                        preserve_focus_hint: Some(false),
                        text: Some(format!("pc = {pc:#x}, cycle count = {}", sim.cycle_count())),
                        all_threads_stopped: None,
                        hit_breakpoint_ids: Some(vec![bp_id]),
                    }))?;
                self.status = ServerStatus::ServeReq;
                break;
            }

            if kind == RunProgKind::SingleStep {
                self.server
                    .send_event(Event::Stopped(events::StoppedEventBody {
                        reason: types::StoppedEventReason::Step,
                        description: Some("Stop at next step".to_string()),
                        thread_id: Some(THREAD_ID),
                        preserve_focus_hint: Some(false),
                        text: Some(format!("pc = {pc:#x}, cycle count = {}", sim.cycle_count())),
                        all_threads_stopped: None,
                        hit_breakpoint_ids: None,
                    }))?;
                self.status = ServerStatus::ServeReq;
                break;
            }
            if sim.cycle_count() >= self.sim_opt.max_cpu_cycle {
                self.server
                    .send_event(Event::Stopped(events::StoppedEventBody {
                        reason: types::StoppedEventReason::Step,
                        description: Some("CPU max cycle reached".to_string()),
                        thread_id: Some(THREAD_ID),
                        preserve_focus_hint: Some(false),
                        text: Some(format!("pc = {pc:#x}, cycle count = {}", sim.cycle_count())),
                        all_threads_stopped: None,
                        hit_breakpoint_ids: None,
                    }))?;
                self.status = ServerStatus::ServeReq;
                break;
            }
        }

        Ok(())
    }

    pub fn start(mut self) -> anyhow::Result<()> {
        self.init()?;

        loop {
            match self.status {
                ServerStatus::ServeReq => {
                    self.serve_req()?;
                }
                ServerStatus::RunProg(kind) => {
                    self.run_prog(kind)?;
                    // After the program is stopped, we need to update the stage info
                    let inner = self
                        .inner
                        .as_mut()
                        .ok_or(anyhow::anyhow!("program data not initialized"))?;
                    inner.stage_info = inner.sim.get_stage_info();
                }
            }
        }
    }
}
