//! Expression in CS:APP HCL language

use quote::ToTokens;
use syn::{parse::Parse, punctuated::Punctuated, Token};

#[derive(Debug, Clone)]
pub struct LValue(pub Punctuated<syn::Ident, Token![.]>);

impl Parse for LValue {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let idents = input.call(
            syn::punctuated::Punctuated::<syn::Ident, syn::Token![.]>::parse_separated_nonempty,
        )?;
        Ok(Self(idents))
    }
}

impl ToTokens for LValue {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}

#[derive(Debug, Clone)]
pub enum PrimaryExpr {
    LVal(LValue),
    LitInt(syn::LitInt),
    LitBool(syn::LitBool),
    Paren(Box<Expr>),
}

impl Parse for PrimaryExpr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(syn::LitInt) {
            Ok(Self::LitInt(input.parse()?))
        } else if lookahead.peek(syn::LitBool) {
            Ok(Self::LitBool(input.parse()?))
        } else if lookahead.peek(syn::token::Paren) {
            let content;
            let _ = syn::parenthesized!(content in input);
            let expr = content.parse()?;
            Ok(Self::Paren(Box::new(expr)))
        } else {
            Ok(Self::LVal(input.parse()?))
        }
    }
}

#[derive(Debug, Clone)]
pub enum UnaryExpr {
    Primary(PrimaryExpr),
    Not(Box<UnaryExpr>),
}

impl Parse for UnaryExpr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![!]) {
            let _ = input.parse::<Token![!]>()?;
            let expr = input.parse()?;
            Ok(Self::Not(Box::new(expr)))
        } else {
            Ok(Self::Primary(input.parse()?))
        }
    }
}

#[derive(Debug, Clone)]
pub enum RelExpr {
    Unary(UnaryExpr),
    Eq(UnaryExpr, UnaryExpr),
    NotEq(UnaryExpr, UnaryExpr),
    In(UnaryExpr, Punctuated<LValue, Token![,]>),
}

impl Parse for RelExpr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lhs = input.parse::<UnaryExpr>()?;
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![==]) {
            let _ = input.parse::<Token![==]>()?;
            let rhs = input.parse()?;
            Ok(Self::Eq(lhs, rhs))
        } else if lookahead.peek(Token![!=]) {
            let _ = input.parse::<Token![!=]>()?;
            let rhs = input.parse()?;
            Ok(Self::NotEq(lhs, rhs))
        } else if lookahead.peek(Token![in]) {
            let _ = input.parse::<Token![in]>()?;
            let items;
            let _ = syn::braced!(items in input);
            let values = items.parse_terminated(LValue::parse, Token![,])?;
            Ok(Self::In(lhs, values))
        } else {
            Ok(Self::Unary(lhs))
        }
    }
}

#[derive(Debug, Clone)]
pub enum LAndExpr {
    Rel(RelExpr),

    /// `expr && expr`
    RelLAnd(RelExpr, Box<LAndExpr>),
}

impl Parse for LAndExpr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lhs = input.parse::<RelExpr>()?;
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![&&]) {
            let _ = input.parse::<Token![&&]>()?;
            let rhs = input.parse()?;
            Ok(Self::RelLAnd(lhs, Box::new(rhs)))
        } else {
            Ok(Self::Rel(lhs))
        }
    }
}

#[derive(Debug, Clone)]
pub enum LOrExpr {
    LAnd(LAndExpr),

    /// `expr || expr`
    LAndOr(LAndExpr, Box<LOrExpr>),
}

impl Parse for LOrExpr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lhs = input.parse::<LAndExpr>()?;
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![||]) {
            let _ = input.parse::<Token![||]>()?;
            let rhs = input.parse()?;
            Ok(Self::LAndOr(lhs, Box::new(rhs)))
        } else {
            Ok(Self::LAnd(lhs))
        }
    }
}

#[derive(Debug, Clone)]
pub struct Expr(LOrExpr);

impl Parse for Expr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let expr = input.parse::<LOrExpr>()?;
        Ok(Self(expr))
    }
}

impl ToTokens for Expr {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}

impl ToTokens for LOrExpr {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::LAnd(expr) => expr.to_tokens(tokens),
            Self::LAndOr(expr, next) => {
                tokens.extend(quote::quote! { #expr || #next });
            }
        }
    }
}

impl ToTokens for LAndExpr {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::Rel(expr) => expr.to_tokens(tokens),
            Self::RelLAnd(expr, next) => {
                tokens.extend(quote::quote! { #expr && #next });
            }
        }
    }
}

impl ToTokens for RelExpr {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::Unary(expr) => expr.to_tokens(tokens),
            Self::Eq(lhs, rhs) => {
                tokens.extend(quote::quote! { #lhs == #rhs });
            }
            Self::NotEq(lhs, rhs) => {
                tokens.extend(quote::quote! { #lhs != #rhs });
            }
            Self::In(lhs, values) => {
                tokens.extend(quote::quote! { crate::dsl::mtc(#lhs, [#values]) });
            }
        }
    }
}

impl ToTokens for UnaryExpr {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::Primary(expr) => expr.to_tokens(tokens),
            Self::Not(expr) => {
                tokens.extend(quote::quote! { !#expr });
            }
        }
    }
}

impl ToTokens for PrimaryExpr {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::LVal(lval) => lval.to_tokens(tokens),
            Self::LitInt(lit) => lit.to_tokens(tokens),
            Self::LitBool(lit) => lit.to_tokens(tokens),
            Self::Paren(expr) => {
                tokens.extend(quote::quote! { ( #expr ) });
            }
        }
    }
}

impl PrimaryExpr {
    fn lvalues(&self) -> Punctuated<LValue, Token![,]> {
        match self {
            PrimaryExpr::LVal(v) => std::iter::once(v.clone()).collect(),
            PrimaryExpr::LitInt(_) => Default::default(),
            PrimaryExpr::LitBool(_) => Default::default(),
            PrimaryExpr::Paren(e) => e.lvalues(),
        }
    }
}

impl UnaryExpr {
    fn lvalues(&self) -> Punctuated<LValue, Token![,]> {
        match self {
            UnaryExpr::Primary(p) => p.lvalues(),
            UnaryExpr::Not(e) => e.lvalues(),
        }
    }
}

impl RelExpr {
    fn lvalues(&self) -> Punctuated<LValue, Token![,]> {
        match self {
            RelExpr::Unary(u) => u.lvalues(),
            RelExpr::Eq(lhs, rhs) | RelExpr::NotEq(lhs, rhs) => {
                let mut lvalues = lhs.lvalues();
                lvalues.extend(rhs.lvalues());
                lvalues
            }
            RelExpr::In(lhs, values) => {
                let mut lvalues = lhs.lvalues();
                lvalues.extend(values.iter().cloned());
                lvalues
            }
        }
    }
}

impl LAndExpr {
    fn lvalues(&self) -> Punctuated<LValue, Token![,]> {
        match self {
            LAndExpr::Rel(r) => r.lvalues(),
            LAndExpr::RelLAnd(lhs, rhs) => {
                let mut lvalues = lhs.lvalues();
                lvalues.extend(rhs.lvalues());
                lvalues
            }
        }
    }
}

impl LOrExpr {
    fn lvalues(&self) -> Punctuated<LValue, Token![,]> {
        match self {
            LOrExpr::LAnd(l) => l.lvalues(),
            LOrExpr::LAndOr(lhs, rhs) => {
                let mut lvalues = lhs.lvalues();
                lvalues.extend(rhs.lvalues());
                lvalues
            }
        }
    }
}

impl Expr {
    /// Get all lvalues in the expression
    pub fn lvalues(&self) -> Punctuated<LValue, Token![,]> {
        self.0.lvalues()
    }
}

impl LValue {
    pub fn map(self, f: impl Fn(LValue) -> LValue + Clone) -> Self {
        f(self)
    }
}

impl PrimaryExpr {
    fn map(self, f: impl Fn(LValue) -> LValue + Clone) -> Self {
        match self {
            PrimaryExpr::LVal(lval) => PrimaryExpr::LVal(lval.map(f)),
            PrimaryExpr::Paren(expr) => PrimaryExpr::Paren(Box::new(expr.map(f))),
            other => other,
        }
    }
}

impl UnaryExpr {
    fn map(self, f: impl Fn(LValue) -> LValue + Clone) -> Self {
        match self {
            UnaryExpr::Primary(p) => UnaryExpr::Primary(p.map(f)),
            UnaryExpr::Not(e) => UnaryExpr::Not(Box::new(e.map(f))),
        }
    }
}

impl RelExpr {
    fn map(self, f: impl Fn(LValue) -> LValue + Clone) -> Self {
        match self {
            RelExpr::Unary(u) => RelExpr::Unary(u.map(f)),
            RelExpr::Eq(lhs, rhs) => RelExpr::Eq(lhs.map(f.clone()), rhs.map(f)),
            RelExpr::NotEq(lhs, rhs) => RelExpr::NotEq(lhs.map(f.clone()), rhs.map(f)),
            RelExpr::In(lhs, values) => {
                RelExpr::In(lhs.map(f.clone()), values.into_iter().map(f).collect())
            }
        }
    }
}

impl LAndExpr {
    fn map(self, f: impl Fn(LValue) -> LValue + Clone) -> Self {
        match self {
            LAndExpr::Rel(r) => LAndExpr::Rel(r.map(f)),
            LAndExpr::RelLAnd(lhs, rhs) => {
                LAndExpr::RelLAnd(lhs.map(f.clone()), Box::new(rhs.map(f)))
            }
        }
    }
}

impl LOrExpr {
    fn map(self, f: impl Fn(LValue) -> LValue + Clone) -> Self {
        match self {
            LOrExpr::LAnd(l) => LOrExpr::LAnd(l.map(f)),
            LOrExpr::LAndOr(lhs, rhs) => LOrExpr::LAndOr(lhs.map(f.clone()), Box::new(rhs.map(f))),
        }
    }
}

impl Expr {
    /// Map all lvalues in the expression
    pub fn map(self, f: impl Fn(LValue) -> LValue + Clone) -> Self {
        Self(self.0.map(f))
    }
}
