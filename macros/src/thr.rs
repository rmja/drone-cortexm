use inflector::Inflector;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream, Result},
    parse_macro_input, token, AttrStyle, Attribute, ExprPath, Ident, LitInt, Token, VisPublic,
    Visibility,
};

struct Input {
    thr: Thr,
    local: Local,
    vtable: Vtable,
    index: Index,
    init: Init,
    sv: Option<Sv>,
    threads: Threads,
}

struct Thr {
    attrs: Vec<Attribute>,
    vis: Visibility,
    ident: Ident,
    tokens: TokenStream2,
}

struct Local {
    attrs: Vec<Attribute>,
    vis: Visibility,
    ident: Ident,
    tokens: TokenStream2,
}

struct Vtable {
    attrs: Vec<Attribute>,
    vis: Visibility,
    ident: Ident,
}

struct Index {
    attrs: Vec<Attribute>,
    vis: Visibility,
    ident: Ident,
}

struct Init {
    attrs: Vec<Attribute>,
    vis: Visibility,
    ident: Ident,
}

struct Sv {
    path: ExprPath,
}

struct Threads {
    threads: Vec<Thread>,
}

enum Thread {
    Reset(ThreadSpec),
    Exception(ThreadSpec),
    Interrupt(u32, ThreadSpec),
}

struct ThreadSpec {
    attrs: Vec<Attribute>,
    vis: Visibility,
    kind: ThreadKind,
    ident: Ident,
}

enum ThreadKind {
    Inner,
    Outer(ExprPath),
    Naked(ExprPath),
}

impl Parse for Input {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut thr = None;
        let mut local = None;
        let mut vtable = None;
        let mut index = None;
        let mut init = None;
        let mut sv = None;
        let mut threads = None;
        while !input.is_empty() {
            let attrs = input.call(Attribute::parse_outer)?;
            let ident = input.parse::<Ident>()?;
            input.parse::<Token![=>]>()?;
            if ident == "thread" {
                if thr.is_none() {
                    thr = Some(Thr::parse(input, attrs)?);
                } else {
                    return Err(input.error("multiple `thread` specifications"));
                }
            } else if ident == "local" {
                if local.is_none() {
                    local = Some(Local::parse(input, attrs)?);
                } else {
                    return Err(input.error("multiple `local` specifications"));
                }
            } else if ident == "vtable" {
                if vtable.is_none() {
                    vtable = Some(Vtable::parse(input, attrs)?);
                } else {
                    return Err(input.error("multiple `vtable` specifications"));
                }
            } else if ident == "index" {
                if index.is_none() {
                    index = Some(Index::parse(input, attrs)?);
                } else {
                    return Err(input.error("multiple `index` specifications"));
                }
            } else if ident == "init" {
                if init.is_none() {
                    init = Some(Init::parse(input, attrs)?);
                } else {
                    return Err(input.error("multiple `init` specifications"));
                }
            } else if attrs.is_empty() && ident == "supervisor" {
                if sv.is_none() {
                    sv = Some(input.parse()?);
                } else {
                    return Err(input.error("multiple `sv` specifications"));
                }
            } else if attrs.is_empty() && ident == "threads" {
                if threads.is_none() {
                    threads = Some(input.parse()?);
                } else {
                    return Err(input.error("multiple `threads` specifications"));
                }
            } else {
                return Err(input.error(format!("unknown key: `{}`", ident)));
            }
            if !input.is_empty() {
                input.parse::<Token![;]>()?;
            }
        }
        Ok(Self {
            thr: thr.ok_or_else(|| input.error("missing `thread` specification"))?,
            local: local.ok_or_else(|| input.error("missing `local` specification"))?,
            vtable: vtable.ok_or_else(|| input.error("missing `vtable` specification"))?,
            index: index.ok_or_else(|| input.error("missing `index` specification"))?,
            init: init.ok_or_else(|| input.error("missing `init` specification"))?,
            sv,
            threads: threads.ok_or_else(|| input.error("missing `threads` specification"))?,
        })
    }
}

impl Thr {
    fn parse(input: ParseStream<'_>, attrs: Vec<Attribute>) -> Result<Self> {
        let vis = input.parse()?;
        let ident = input.parse()?;
        let input2;
        braced!(input2 in input);
        let tokens = input2.parse()?;
        Ok(Self { attrs, vis, ident, tokens })
    }
}

impl Local {
    fn parse(input: ParseStream<'_>, attrs: Vec<Attribute>) -> Result<Self> {
        let vis = input.parse()?;
        let ident = input.parse()?;
        let input2;
        braced!(input2 in input);
        let tokens = input2.parse()?;
        Ok(Self { attrs, vis, ident, tokens })
    }
}

impl Vtable {
    fn parse(input: ParseStream<'_>, attrs: Vec<Attribute>) -> Result<Self> {
        let vis = input.parse()?;
        let ident = input.parse()?;
        Ok(Self { attrs, vis, ident })
    }
}

impl Index {
    fn parse(input: ParseStream<'_>, attrs: Vec<Attribute>) -> Result<Self> {
        let vis = input.parse()?;
        let ident = input.parse()?;
        Ok(Self { attrs, vis, ident })
    }
}

impl Init {
    fn parse(input: ParseStream<'_>, attrs: Vec<Attribute>) -> Result<Self> {
        let vis = input.parse()?;
        let ident = input.parse()?;
        Ok(Self { attrs, vis, ident })
    }
}

impl Parse for Sv {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let path = input.parse()?;
        Ok(Self { path })
    }
}

impl Parse for Threads {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let input2;
        braced!(input2 in input);
        let mut threads = Vec::new();
        while !input2.is_empty() {
            let attrs = input2.call(Attribute::parse_outer)?;
            let ident = input2.parse::<Ident>()?;
            input2.parse::<Token![=>]>()?;
            if attrs.is_empty() && ident == "exceptions" {
                let input3;
                braced!(input3 in input2);
                while !input3.is_empty() {
                    let attrs = input3.call(Attribute::parse_outer)?;
                    let vis = input3.parse()?;
                    let kind = input3.parse()?;
                    let ident = input3.parse()?;
                    threads.push(Thread::Exception(ThreadSpec { attrs, vis, kind, ident }));
                    if !input3.is_empty() {
                        input3.parse::<Token![;]>()?;
                    }
                }
            } else if attrs.is_empty() && ident == "interrupts" {
                let input3;
                braced!(input3 in input2);
                while !input3.is_empty() {
                    let attrs = input3.call(Attribute::parse_outer)?;
                    let num = input3.parse::<LitInt>()?.base10_parse()?;
                    input3.parse::<Token![:]>()?;
                    let vis = input3.parse()?;
                    let kind = input3.parse()?;
                    let ident = input3.parse()?;
                    threads.push(Thread::Interrupt(num, ThreadSpec { attrs, vis, kind, ident }));
                    if !input3.is_empty() {
                        input3.parse::<Token![;]>()?;
                    }
                }
            } else {
                return Err(input2.error(format!("Unexpected ident `{}`", ident)));
            }
            if !input2.is_empty() {
                input2.parse::<Token![;]>()?;
            }
        }
        Ok(Self { threads })
    }
}

impl Thread {
    fn reset() -> Self {
        Self::Reset(ThreadSpec {
            attrs: vec![Attribute {
                pound_token: Token![#](Span::call_site()),
                style: AttrStyle::Outer,
                bracket_token: token::Bracket(Span::call_site()),
                path: format_ident!("doc").into(),
                tokens: quote!(= "Reset thread token."),
            }],
            vis: Visibility::Public(VisPublic { pub_token: Token![pub](Span::call_site()) }),
            kind: ThreadKind::Inner,
            ident: format_ident!("reset"),
        })
    }
}

impl Parse for ThreadKind {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        match input.fork().parse::<Ident>() {
            Ok(ident) if ident == "outer" => {
                input.parse::<Ident>()?;
                let input2;
                parenthesized!(input2 in input);
                let path = input2.parse()?;
                Ok(Self::Outer(path))
            }
            Ok(ident) if ident == "naked" => {
                input.parse::<Ident>()?;
                let input2;
                parenthesized!(input2 in input);
                let path = input2.parse()?;
                Ok(Self::Naked(path))
            }
            _ => Ok(Self::Inner),
        }
    }
}

pub fn proc_macro(input: TokenStream) -> TokenStream {
    let Input { thr, local, vtable, index, init, sv, threads } = parse_macro_input!(input as Input);
    let Threads { mut threads } = threads;
    threads.insert(0, Thread::reset());
    let threads = enumerate_threads(threads);
    let def_core_thr = def_core_thr(&thr, &local);
    let def_vtable = def_vtable(&thr, &vtable, &threads);
    let def_array = def_array(&thr, &threads);
    let def_index = def_index(&thr, &index, &sv, &threads);
    let def_init = def_init(&index, &init);
    let expanded = quote! {
        #def_core_thr
        #def_vtable
        #def_array
        #def_index
        #def_init
        ::drone_cortexm::reg::assert_taken!("scb_ccr");
        ::drone_cortexm::reg::assert_taken!("mpu_type");
        ::drone_cortexm::reg::assert_taken!("mpu_ctrl");
        ::drone_cortexm::reg::assert_taken!("mpu_rnr");
        ::drone_cortexm::reg::assert_taken!("mpu_rbar");
        ::drone_cortexm::reg::assert_taken!("mpu_rasr");
    };
    expanded.into()
}

fn enumerate_threads(threads: Vec<Thread>) -> Vec<(Option<usize>, Thread)> {
    let mut counter = 0;
    threads
        .into_iter()
        .map(|thread| match &thread {
            Thread::Reset(spec) | Thread::Exception(spec) | Thread::Interrupt(_, spec) => {
                let ThreadSpec { kind, .. } = spec;
                match kind {
                    ThreadKind::Inner | ThreadKind::Outer(_) => {
                        let idx = counter;
                        counter += 1;
                        (Some(idx), thread)
                    }
                    ThreadKind::Naked(_) => (None, thread),
                }
            }
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn def_vtable(thr: &Thr, vtable: &Vtable, threads: &[(Option<usize>, Thread)]) -> TokenStream2 {
    let Vtable { attrs: vtable_attrs, vis: vtable_vis, ident: vtable_ident } = vtable;
    let mut tokens = Vec::new();
    let mut vtable_tokens = Vec::new();
    let mut vtable_ctor_tokens = Vec::new();
    let mut vtable_ctor_default_tokens = Vec::new();
    for (idx, thread) in threads {
        match thread {
            Thread::Reset(_) => {}
            Thread::Exception(spec) | Thread::Interrupt(_, spec) => {
                let ThreadSpec { kind, ident, .. } = spec;
                let field_ident = format_ident!("{}", ident.to_string().to_snake_case());
                let (handler, path) = def_handler(thr, idx, kind);
                tokens.push(handler);
                vtable_ctor_tokens.push(quote! {
                    #field_ident: Some(#path)
                });
                if let Thread::Interrupt(num, _) = thread {
                    let num = *num as usize;
                    if vtable_tokens.len() < num + 1 {
                        vtable_tokens.resize(num + 1, None);
                    }
                    vtable_tokens[num] = Some(quote! {
                        #field_ident: Option<unsafe extern "C" fn()>
                    });
                    vtable_ctor_default_tokens.push(quote! {
                        #field_ident: None
                    });
                }
            }
        }
    }
    let vtable_tokens = vtable_tokens
        .into_iter()
        .enumerate()
        .map(|(i, tokens)| {
            tokens.unwrap_or_else(|| {
                let field_ident = format_ident!("_int{}", i);
                vtable_ctor_default_tokens.push(quote! {
                    #field_ident: None
                });
                quote! {
                    #field_ident: Option<unsafe extern "C" fn()>
                }
            })
        })
        .collect::<Vec<_>>();
    quote! {
        #(#vtable_attrs)*
        #[allow(dead_code)]
        #vtable_vis struct #vtable_ident {
            reset: unsafe extern "C" fn() -> !,
            nmi: Option<unsafe extern "C" fn()>,
            hard_fault: Option<unsafe extern "C" fn()>,
            mem_manage: Option<unsafe extern "C" fn()>,
            bus_fault: Option<unsafe extern "C" fn()>,
            usage_fault: Option<unsafe extern "C" fn()>,
            #[cfg(feature = "security-extension")]
            secure_fault: Option<unsafe extern "C" fn()>,
            #[cfg(not(feature = "security-extension"))]
            _reserved0: [usize; 1],
            _reserved1: [usize; 3],
            sv_call: Option<unsafe extern "C" fn()>,
            debug: Option<unsafe extern "C" fn()>,
            _reserved2: [usize; 1],
            pend_sv: Option<unsafe extern "C" fn()>,
            sys_tick: Option<unsafe extern "C" fn()>,
            #(#vtable_tokens),*
        }

        impl #vtable_ident {
            /// Creates a new vector table.
            pub const fn new(reset: unsafe extern "C" fn() -> !) -> Self {
                Self {
                    #(#vtable_ctor_tokens,)*
                    ..Self {
                        reset,
                        nmi: None,
                        hard_fault: None,
                        mem_manage: None,
                        bus_fault: None,
                        usage_fault: None,
                        #[cfg(feature = "security-extension")]
                        secure_fault: None,
                        #[cfg(not(feature = "security-extension"))]
                        _reserved0: [0; 1],
                        _reserved1: [0; 3],
                        sv_call: None,
                        debug: None,
                        _reserved2: [0; 1],
                        pend_sv: None,
                        sys_tick: None,
                        #(#vtable_ctor_default_tokens,)*
                    }
                }
            }
        }

        #(#tokens)*
    }
}

fn def_array(thr: &Thr, threads: &[(Option<usize>, Thread)]) -> TokenStream2 {
    let Thr { ident: thr_ident, .. } = thr;
    let mut array_tokens = Vec::new();
    for (idx, _) in threads {
        if let Some(idx) = idx {
            array_tokens.push(quote! {
                #thr_ident::new(#idx)
            });
        }
    }
    let array_len = array_tokens.len();
    quote! {
        static mut THREADS: [#thr_ident; #array_len] = [#(#array_tokens),*];
    }
}

fn def_index(
    thr: &Thr,
    index: &Index,
    sv: &Option<Sv>,
    threads: &[(Option<usize>, Thread)],
) -> TokenStream2 {
    let Index { attrs: index_attrs, vis: index_vis, ident: index_ident } = index;
    let mut tokens = Vec::new();
    let mut index_tokens = Vec::new();
    let mut index_ctor_tokens = Vec::new();
    for (idx, thread) in threads {
        if let Some((new_tokens, new_index_tokens, new_index_ctor_tokens)) =
            def_thr_token(thr, sv, idx, thread)
        {
            tokens.push(new_tokens);
            index_tokens.push(new_index_tokens);
            index_ctor_tokens.push(new_index_ctor_tokens);
        }
    }
    quote! {
        #(#index_attrs)*
        #index_vis struct #index_ident {
            #(#index_tokens),*
        }

        unsafe impl ::drone_core::token::Token for #index_ident {
            #[inline]
            unsafe fn take() -> Self {
                Self {
                    #(#index_ctor_tokens),*
                }
            }
        }

        unsafe impl ::drone_cortexm::thr::ThrTokens for #index_ident {}

        #(#tokens)*
    }
}

fn def_init(index: &Index, init: &Init) -> TokenStream2 {
    let Init { attrs: init_attrs, vis: init_vis, ident: init_ident } = init;
    let Index { ident: index_ident, .. } = index;
    quote! {
        #(#init_attrs)*
        #init_vis struct #init_ident {
            __priv: (),
        }

        unsafe impl ::drone_core::token::Token for #init_ident {
            #[inline]
            unsafe fn take() -> Self {
                Self {
                    __priv: (),
                }
            }
        }

        unsafe impl ::drone_cortexm::thr::ThrsInitToken for #init_ident {
            type ThrTokens = #index_ident;
        }
    }
}

fn def_core_thr(thr: &Thr, local: &Local) -> TokenStream2 {
    let Thr { attrs: thr_attrs, vis: thr_vis, ident: thr_ident, tokens: thr_tokens } = thr;
    let Local { attrs: local_attrs, vis: local_vis, ident: local_ident, tokens: local_tokens } =
        local;
    quote! {
        ::drone_core::thr! {
            array => THREADS;

            #(#thr_attrs)*
            thread => #thr_vis #thr_ident { #thr_tokens };

            #(#local_attrs)*
            local => #local_vis #local_ident { #local_tokens };
        }
    }
}

fn def_thr_token(
    thr: &Thr,
    sv: &Option<Sv>,
    idx: &Option<usize>,
    thread: &Thread,
) -> Option<(TokenStream2, TokenStream2, TokenStream2)> {
    let Thr { ident: thr_ident, .. } = thr;
    match thread {
        Thread::Reset(spec) | Thread::Exception(spec) | Thread::Interrupt(_, spec) => {
            let ThreadSpec { attrs, vis, kind, ident } = spec;
            match kind {
                ThreadKind::Inner | ThreadKind::Outer(_) => {
                    let mut tokens = Vec::new();
                    let field_ident = format_ident!("{}", ident.to_string().to_snake_case());
                    let struct_ident = format_ident!("{}", ident.to_string().to_pascal_case());
                    tokens.push(quote! {
                        #(#attrs)*
                        #[derive(Clone, Copy)]
                        #vis struct #struct_ident {
                            __priv: (),
                        }

                        unsafe impl ::drone_core::token::Token for #struct_ident {
                            #[inline]
                            unsafe fn take() -> Self {
                                #struct_ident {
                                    __priv: (),
                                }
                            }
                        }

                        unsafe impl ::drone_core::thr::ThrToken for #struct_ident {
                            type Thr = #thr_ident;

                            const THR_IDX: usize = #idx;
                        }
                    });
                    if let Some(Sv { path: sv_path }) = sv {
                        tokens.push(quote! {
                            impl ::drone_cortexm::thr::ThrSv for #struct_ident {
                                type Sv = #sv_path;
                            }
                        });
                    }
                    if let Thread::Interrupt(num, _) = thread {
                        let num = *num as usize;
                        let nvic_block = format_ident!("NvicBlock{}", num / 32);
                        tokens.push(quote! {
                            impl ::drone_cortexm::thr::IntToken for #struct_ident {
                                type NvicBlock = ::drone_cortexm::map::thr::#nvic_block;

                                const INT_NUM: usize = #num;
                            }
                        });
                    }
                    Some((
                        quote!(#(#tokens)*),
                        quote! {
                            #(#attrs)*
                            #vis #field_ident: #struct_ident
                        },
                        quote! {
                            #field_ident: ::drone_core::token::Token::take()
                        },
                    ))
                }
                ThreadKind::Naked(_) => None,
            }
        }
    }
}

fn def_handler(thr: &Thr, idx: &Option<usize>, kind: &ThreadKind) -> (TokenStream2, TokenStream2) {
    let Thr { ident: thr_ident, .. } = thr;
    match kind {
        ThreadKind::Inner => {
            let ident = format_ident!("thr_handler_{}", idx.unwrap());
            (
                quote! {
                    unsafe extern "C" fn #ident() {
                        unsafe {
                            ::drone_cortexm::thr::thread_resume::<#thr_ident>(#idx);
                        }
                    }
                },
                quote!(#ident),
            )
        }
        ThreadKind::Outer(path) => {
            let ident = format_ident!("thr_handler_{}_outer", idx.unwrap());
            (
                quote! {
                    unsafe extern "C" fn #ident() {
                        unsafe {
                            ::drone_cortexm::thr::thread_call::<#thr_ident>(#idx, #path);
                        }
                    }
                },
                quote!(#ident),
            )
        }
        ThreadKind::Naked(path) => (quote!(), quote!(#path)),
    }
}
