use anyhow::{bail, ensure};
use dap::prelude::*;
use serde::Deserialize;
use std::{
    io::{BufReader, BufWriter, Read, Write},
    path::PathBuf,
};
use y86_sim::framework::CpuSim;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunProgKind {
    SingleStep,
    Run,
}

enum ServerStatus {
    /// The server is running the program so it does not accept any request.
    RunProg(RunProgKind),
    /// The program execution is stopped and the server is waiting for client request.
    ServeReq,
}

pub struct DebugServer<R: Read, W: Write> {
    sim: Option<y86_sim::framework::PipeSim<y86_sim::Arch>>,
    /// Lines of breakpoints
    breakpoints: Vec<types::Breakpoint>,
    /// Path of the source file (this debugger only supports single source file)
    source_path: Option<PathBuf>,
    source_info: Option<y86_sim::SourceInfo>,
    source_name: Option<String>,
    server: dap::server::Server<R, W>,
    status: ServerStatus,
}

const THREAD_ID: i64 = 1;
const STACK_FRAME_ID: i64 = 1;
const REG_SCOPE_VAR_REF: i64 = 1;

#[derive(Deserialize)]
struct LaunchOption {
    program: String,
}

impl<R: Read, W: Write> DebugServer<R, W> {
    pub fn new(input: R, output: W) -> Self {
        Self {
            sim: None,
            server: dap::server::Server::new(BufReader::new(input), BufWriter::new(output)),
            breakpoints: Vec::new(),
            source_path: None,
            source_info: None,
            source_name: None,
            status: ServerStatus::ServeReq,
        }
    }

    fn get_src_info(&self) -> &y86_sim::SourceInfo {
        self.source_info
            .as_ref()
            .expect("source info not initialized")
    }

    fn init_program(&mut self, program: PathBuf) -> anyhow::Result<()> {
        tracing::info!("initializing program: {}", program.display());

        let src = std::fs::read_to_string(&program)?;
        let a = y86_sim::assemble(&src, y86_sim::AssembleOption::default())?;
        let sim = y86_sim::framework::PipeSim::new(a.obj.init_mem(), false);
        self.sim = Some(sim);
        self.source_name = Some(program.file_name().unwrap().to_string_lossy().to_string());
        self.source_path = Some(program);
        self.source_info = Some(a.source);

        Ok(())
    }

    fn main_source(&self) -> types::Source {
        types::Source {
            name: self.source_name.clone(),
            path: self.source_path.clone().map(|p| p.display().to_string()),
            source_reference: None,
            presentation_hint: Some(types::PresentationHint::Normal),
            origin: None,
            sources: None,
            adapter_data: None,
            checksums: None,
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
                ServerStatus::RunProg(RunProgKind::Run),
            )),
            Command::SetBreakpoints(args) => {
                let Some(breakpoints) = &args.breakpoints else {
                    bail!("missing breakpoints");
                };

                let source_path = PathBuf::from(
                    args.source
                        .path
                        .clone()
                        .ok_or(anyhow::anyhow!("missing source name"))?,
                );
                if Some(source_path) != self.source_path {
                    bail!("source path mismatch");
                }

                let srcinfo = self.get_src_info();

                let bps: Vec<types::Breakpoint> = breakpoints
                    .into_iter()
                    .map(|b| {
                        let verified = true;
                        let ln = srcinfo.get_line(b.line).unwrap();
                        let message = ln.addr.map(|a| format!("addr: {:#x}", a));

                        let Some(addr) = ln.addr else {
                            return types::Breakpoint::default();
                        };
                        let Some(_) = &ln.inst else {
                            return types::Breakpoint::default();
                        };

                        types::Breakpoint {
                            // we use the address as the id
                            id: Some(addr as i64),
                            verified,
                            message,
                            source: Some(self.main_source()),
                            line: Some(b.line),
                            column: None,
                            end_line: None,
                            end_column: None,
                            instruction_reference: None,
                            offset: None,
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
                let sim = self
                    .sim
                    .as_ref()
                    .ok_or(anyhow::anyhow!("simulator not initialized"))?;
                let srcinfo = self
                    .source_info
                    .as_ref()
                    .ok_or(anyhow::anyhow!("source info not initialized"))?;
                // currently we don't have stack trace, thus we return a single frame
                Ok((
                    req.success(ResponseBody::StackTrace(responses::StackTraceResponse {
                        stack_frames: vec![types::StackFrame {
                            id: STACK_FRAME_ID,
                            name: "current".to_string(),
                            source: Some(self.main_source()),
                            line: srcinfo
                                .get_line_number_by_addr(sim.program_counter())
                                .unwrap_or_default(),
                            column: 0,
                            end_line: None,
                            end_column: None,
                            can_restart: Some(false),
                            instruction_pointer_reference: None,
                            module_id: None,
                            presentation_hint: Some(types::StackFramePresentationhint::Normal),
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
                        scopes: vec![types::Scope {
                            name: "Registers".to_string(),
                            presentation_hint: Some(types::ScopePresentationhint::Registers),
                            variables_reference: REG_SCOPE_VAR_REF,
                            named_variables: None,
                            indexed_variables: None,
                            expensive: false,
                            source: Some(self.main_source()),
                            line: None,
                            column: None,
                            end_line: None,
                            end_column: None,
                        }],
                    })),
                    ServerStatus::ServeReq,
                ))
            }
            Command::Variables(args) => {
                if args.variables_reference != REG_SCOPE_VAR_REF {
                    bail!("invalid variable reference");
                }
                let sim = self
                    .sim
                    .as_ref()
                    .ok_or(anyhow::anyhow!("simulator not initialized"))?;
                let regs = sim.registers();
                let vars = regs
                    .iter()
                    .map(|(reg, val)| {
                        let value = format!("{:#x}", val);
                        types::Variable {
                            name: y86_sim::isa::reg_code::name_of(*reg).to_string(),
                            value,
                            type_field: None,
                            presentation_hint: Some(types::VariablePresentationHint {
                                kind: Some(types::VariablePresentationHintKind::Data),
                                attributes: None,
                                visibility: None,
                                lazy: Some(false),
                            }),
                            evaluate_name: None,
                            variables_reference: 0,
                            named_variables: None,
                            indexed_variables: None,
                            memory_reference: None,
                        }
                    })
                    .collect();
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
        // in a base message with a appropriate seq number, so you don't have to keep track of that yourself
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
        let sim = self
            .sim
            .as_mut()
            .ok_or(anyhow::anyhow!("simulator not initialized"))?;

        // start the simulation loop
        loop {
            if sim.is_terminate() {
                tracing::info!("program terminated");
                self.server
                    .send_event(Event::Stopped(events::StoppedEventBody {
                        reason: types::StoppedEventReason::Pause,
                        description: Some(format!("Pause on termination")),
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

            let srcinfo = self
                .source_info
                .as_ref()
                .ok_or(anyhow::anyhow!("source info not initialized"))?;
            let pc = sim.program_counter();

            if let Some(bp) = self.breakpoints.iter().find(|bp| {
                let Some(bp_ln) = bp.line else { return false };
                srcinfo
                    .get_line_number_by_addr(pc)
                    .map(|ln| ln == bp_ln)
                    .unwrap_or(false)
            }) {
                let bp_id = bp.id.ok_or(anyhow::anyhow!("breakpoint id not set"))?;
                tracing::trace!("hit breakpoint: line = {:?}", bp.line);
                self.server
                    .send_event(Event::Stopped(events::StoppedEventBody {
                        reason: types::StoppedEventReason::Breakpoint,
                        description: Some(format!("Stop at breakpoint")),
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
                        description: Some(format!("Stop at next step")),
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
                }
            }
        }
    }
}
