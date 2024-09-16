use crate::expr;
use expr::LValue;
use syn::{parse::Parse, punctuated::Punctuated, Token};

struct AtoB {
    a: syn::Ident,
    b: syn::Ident,
}

impl Parse for AtoB {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let a = input.parse()?;
        input.parse::<Token![=>]>()?;
        let b = input.parse()?;
        Ok(Self { a, b })
    }
}

#[derive(Debug, Default)]
pub struct StageAlias(pub Vec<(syn::Ident, syn::Ident)>);

impl Parse for StageAlias {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let args = Punctuated::<AtoB, Token![,]>::parse_terminated(input)?;
        Ok(Self(
            args.iter()
                .map(|arg| (arg.b.clone(), arg.a.clone()))
                .collect(),
        ))
    }
}

/// e.g. `imem.error => NOP`
#[derive(Debug)]
pub struct Case {
    pub tunnel: Option<syn::Ident>,
    pub condition: expr::Expr,
    pub value: expr::Expr,
}

impl Case {
    pub fn lvalues(&self) -> Punctuated<LValue, Token![,]> {
        let mut values = self.condition.lvalues();
        values.extend(self.value.lvalues());
        values
    }
}

impl Parse for Case {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attr = input.call(syn::Attribute::parse_outer)?;
        let condition: expr::Expr = input.parse()?;
        let _ = input.parse::<syn::Token![=>]>()?;
        let value: expr::Expr = input.parse()?;

        let tunnel = attr.iter().find_map(|attr| {
            if attr.path().is_ident("tunnel") {
                let tunnel_name: syn::Ident = attr.parse_args().unwrap();
                Some(tunnel_name)
            } else {
                None
            }
        });

        Ok(Self {
            tunnel,
            condition,
            value,
        })
    }
}

#[derive(Debug)]
pub struct SignalSwitch(pub Punctuated<Case, syn::Token![;]>);

impl Parse for SignalSwitch {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let cases;
        let _ = syn::bracketed!(cases in input);
        let cases = cases.parse_terminated(Case::parse, syn::Token![;])?;
        Ok(Self(cases))
    }
}

#[derive(Debug)]
pub struct SignalSourceExpr {
    pub tunnel: Option<syn::Ident>,
    pub expr: expr::Expr,
}

impl Parse for SignalSourceExpr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attr = input.call(syn::Attribute::parse_outer)?;
        let expr: expr::Expr = input.parse()?;
        let tunnel = attr.iter().find_map(|attr| {
            if attr.path().is_ident("tunnel") {
                let tunnel_name: syn::Ident = attr.parse_args().unwrap();
                Some(tunnel_name)
            } else {
                None
            }
        });
        Ok(Self { tunnel, expr })
    }
}

#[derive(Debug)]
pub enum SignalSource {
    Switch(SignalSwitch),
    Expr(SignalSourceExpr),
}

impl Parse for SignalSource {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(syn::token::Bracket) {
            let switch = input.parse()?;
            Ok(Self::Switch(switch))
        } else {
            let expr = input.parse()?;
            Ok(Self::Expr(expr))
        }
    }
}

impl SignalSource {
    pub fn lvalues(&self) -> Punctuated<LValue, Token![,]> {
        match self {
            Self::Switch(switch) => switch.0.iter().flat_map(|case| case.lvalues()).collect(),
            Self::Expr(expr) => expr.expr.lvalues(),
        }
    }
}

#[derive(Debug)]
pub struct SignalDest {
    pub tunnel: Option<syn::Ident>,
    pub dest: LValue,
}

impl Parse for SignalDest {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attr = input.call(syn::Attribute::parse_outer)?;
        let dest: LValue = input.parse()?;

        let tunnel = attr.iter().find_map(|attr| {
            if attr.path().is_ident("tunnel") {
                let tunnel_name: syn::Ident = attr.parse_args().unwrap();
                Some(tunnel_name)
            } else {
                None
            }
        });

        Ok(Self { tunnel, dest })
    }
}

/// e.g.
/// ```plain
/// u8 f_icode = [
///     imem.error => NOP;
///     1 => imem.icode;
/// ] -> d.icode;
///
/// u8 f_icode = [
///     imem.error => NOP;
///     1 => imem.icode
/// ];
///
/// u8 f_icode = [
///     imem.error => NOP;
/// ];
///
/// u8 f_icode = A in [B, C, D];
/// ```
#[derive(Debug)]
pub struct SignalDef {
    /// name of the variable
    pub name: syn::Ident,
    /// type of the variable
    pub typ: syn::Type,
    pub source: SignalSource,
    pub destinations: Vec<SignalDest>,
}

impl Parse for SignalDef {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let typ: syn::Type = input.parse()?;
        let name: syn::Ident = input.parse()?;
        let _ = input.parse::<syn::Token![=]>()?;
        let source: SignalSource = input.parse()?;
        let destinations = if let Ok(_) = input.parse::<syn::Token![->]>() {
            if let Ok(dest) = input.parse() {
                vec![dest]
            } else {
                let items;
                let _ = syn::parenthesized!(items in input);
                items
                    .parse_terminated(SignalDest::parse, syn::Token![,])?
                    .into_iter()
                    .collect()
            }
        } else {
            Vec::new()
        };
        let _ = input.parse::<syn::Token![;]>()?;

        Ok(Self {
            name,
            typ,
            source,
            destinations,
        })
    }
}
