use drone_macros_core::{parse_extern_name, parse_own_name};
use failure::{err_msg, Error};
use inflector::Inflector;
use proc_macro::TokenStream;
use quote::Tokens;
use syn::{parse_token_trees, Ident, IntTy, Lit, Token, TokenTree};

pub(crate) fn vtable(input: TokenStream) -> Result<Tokens, Error> {
  let input = parse_token_trees(&input.to_string()).map_err(err_msg)?;
  let mut input = input.into_iter();
  let mut threads = Vec::new();
  let (attrs, name) = parse_own_name(&mut input)?;
  let (tokens_attrs, tokens_name) = parse_own_name(&mut input)?;
  let (static_attrs, static_name) = parse_own_name(&mut input)?;
  let thread_local = parse_extern_name(&mut input)?;
  let name =
    name.ok_or_else(|| format_err!("Unexpected end of macro invokation"))?;
  let tokens_name = tokens_name
    .ok_or_else(|| format_err!("Unexpected end of macro invokation"))?;
  let static_name = static_name
    .ok_or_else(|| format_err!("Unexpected end of macro invokation"))?;
  let thread_local = thread_local
    .ok_or_else(|| format_err!("Unexpected end of macro invokation"))?;
  'outer: loop {
    let mut attrs = Vec::new();
    loop {
      match input.next() {
        Some(TokenTree::Token(Token::DocComment(ref string)))
          if string.starts_with("///") =>
        {
          let string = string.trim_left_matches("///");
          attrs.push(quote!(#[doc = #string]));
        }
        Some(TokenTree::Token(Token::Pound)) => match input.next() {
          Some(TokenTree::Delimited(delimited)) => {
            attrs.push(quote!(# #delimited))
          }
          token => Err(format_err!("Invalid tokens after `#`: {:?}", token))?,
        },
        Some(TokenTree::Token(Token::Ident(name))) => {
          match input.next() {
            Some(TokenTree::Token(Token::Semi)) => (),
            token => {
              Err(format_err!("Invalid token after `{}`: {:?}", name, token))?
            }
          }
          threads.push((attrs, None, name));
          break;
        }
        Some(TokenTree::Token(Token::Literal(Lit::Int(
          number,
          IntTy::Unsuffixed,
        )))) => {
          match input.next() {
            Some(TokenTree::Token(Token::Colon)) => (),
            token => {
              Err(format_err!("Invalid token after `{}`: {:?}", number, token))?
            }
          }
          let name = match input.next() {
            Some(TokenTree::Token(Token::Ident(name))) => name,
            token => Err(format_err!(
              "Invalid token after `{}:`: {:?}",
              number,
              token
            ))?,
          };
          match input.next() {
            Some(TokenTree::Token(Token::Semi)) => (),
            token => {
              Err(format_err!("Invalid token after `{}`: {:?}", name, token))?
            }
          }
          threads.push((attrs, Some(number), name));
          break;
        }
        None => break 'outer,
        token => Err(format_err!("Invalid token: {:?}", token))?,
      }
    }
  }

  let irq_count = threads
    .iter()
    .filter_map(|&(_, number, _)| number)
    .max()
    .map(|x| x + 1)
    .unwrap_or(0);
  let mut irq_name = (0..irq_count)
    .map(|n| Ident::new(format!("_irq{}", n)))
    .collect::<Vec<_>>();
  let thread_count = Lit::Int(threads.len() as u64 + 1, IntTy::Unsuffixed);
  let mut thread_tokens = Vec::new();
  let mut thread_ctor_tokens = Vec::new();
  let mut thread_static_tokens = Vec::new();
  let mut thread_tokens_struct_tokens = Vec::new();
  let mut thread_tokens_impl_tokens = Vec::new();
  thread_static_tokens.push(quote!(#thread_local::new(0)));
  for (index, thread) in threads.into_iter().enumerate() {
    let (
      tokens,
      ctor_tokens,
      static_tokens,
      tokens_struct_tokens,
      tokens_impl_tokens,
    ) = parse_thread(index, thread, &thread_local, &mut irq_name)?;
    thread_tokens.push(tokens);
    thread_ctor_tokens.push(ctor_tokens);
    thread_static_tokens.push(static_tokens);
    thread_tokens_struct_tokens.push(tokens_struct_tokens);
    thread_tokens_impl_tokens.push(tokens_impl_tokens);
  }
  let irq_name = &irq_name;

  Ok(quote! {
    #[allow(unused_imports)]
    use ::core::ops::Deref;
    #[allow(unused_imports)]
    use ::core::marker::PhantomData;
    #[allow(unused_imports)]
    use ::drone_cortex_m::drivers;
    #[allow(unused_imports)]
    use ::drone_cortex_m::thread::irq::*;
    #[allow(unused_imports)]
    use ::drone_cortex_m::thread::prelude::*;
    #[allow(unused_imports)]
    use ::drone_cortex_m::thread::vtable::{Handler, Reserved, ResetHandler};

    #(#attrs)*
    #[allow(dead_code)]
    pub struct #name {
      reset: ResetHandler,
      nmi: Option<Handler>,
      hard_fault: Option<Handler>,
      mem_manage: Option<Handler>,
      bus_fault: Option<Handler>,
      usage_fault: Option<Handler>,
      _reserved0: [Reserved; 4],
      sv_call: Option<Handler>,
      debug: Option<Handler>,
      _reserved1: [Reserved; 1],
      pend_sv: Option<Handler>,
      sys_tick: Option<Handler>,
      #(
        #irq_name: Option<Handler>,
      )*
    }

    impl #name {
      /// Creates a new vector table.
      #[inline(always)]
      pub const fn new(reset: ResetHandler) -> #name {
        #name {
          #(#thread_ctor_tokens,)*
          ..#name {
            reset,
            nmi: None,
            hard_fault: None,
            mem_manage: None,
            bus_fault: None,
            usage_fault: None,
            _reserved0: [Reserved::Vector; 4],
            sv_call: None,
            debug: None,
            _reserved1: [Reserved::Vector; 1],
            pend_sv: None,
            sys_tick: None,
            #(
              #irq_name: None,
            )*
          }
        }
      }
    }

    #(#tokens_attrs)*
    pub struct #tokens_name {
      #(#thread_tokens_struct_tokens),*
    }

    impl ThreadTokens for #tokens_name {
      type Thread = #thread_local;
      type Token = drivers::nvic::Nvic;

      fn new(_token: Self::Token) -> Self {
        Self {
          #(#thread_tokens_impl_tokens),*
        }
      }
    }

    #(#static_attrs)*
    static mut #static_name: [#thread_local; #thread_count] = [
      #(#thread_static_tokens),*
    ];

    #(#thread_tokens)*
  })
}

fn parse_thread(
  index: usize,
  (attrs, number, name): (Vec<Tokens>, Option<u64>, Ident),
  thread_local: &Ident,
  irq_name: &mut [Ident],
) -> Result<(Tokens, Tokens, Tokens, Tokens, Tokens), Error> {
  let field_name = Ident::new(name.as_ref().to_snake_case());
  let struct_name = Ident::new(name.as_ref().to_pascal_case());
  let index = Lit::Int(index as u64 + 1, IntTy::Unsuffixed);
  let attrs = &attrs;

  if let Some(number) = number {
    irq_name[number as usize] = field_name.clone();
  }

  let interrupt = match number {
    Some(number) => {
      let irq_trait = Ident::new(format!("Irq{}", number));
      let bundle = Ident::new(format!("IrqBundle{}", number / 32));
      let number = Lit::Int(number, IntTy::Unsuffixed);
      quote! {
        impl<T: ThreadTag> IrqToken<T> for #struct_name<T> {
          type Bundle = #bundle;

          const IRQ_NUMBER: usize = #number;
        }

        impl<T: ThreadTag> #irq_trait<T> for #struct_name<T> {}
      }
    }
    None => {
      let irq_trait = Ident::new(format!("Irq{}", struct_name));
      quote! {
        impl<T: ThreadTag> #irq_trait<T> for #struct_name<T> {}
      }
    }
  };

  Ok((
    quote! {
      #(#attrs)*
      #[derive(Clone, Copy)]
      pub struct #struct_name<T: ThreadTag> {
        _tag: PhantomData<T>,
      }

      impl<T: ThreadTag> #struct_name<T> {
        #[inline(always)]
        unsafe fn new() -> Self {
          Self { _tag: PhantomData }
        }
      }

      impl<T: ThreadTag> ThreadToken<T> for #struct_name<T> {
        type Thread = #thread_local;

        const THREAD_NUMBER: usize = #index;
      }

      impl<T: ThreadTag> Deref for #struct_name<T> {
        type Target = #thread_local;

        #[inline(always)]
        fn deref(&self) -> &#thread_local {
          self.as_thread()
        }
      }

      impl From<#struct_name<Ctt>> for #struct_name<Ttt> {
        #[inline(always)]
        fn from(_token: #struct_name<Ctt>) -> Self {
          unsafe { Self::new() }
        }
      }

      impl From<#struct_name<Ctt>> for #struct_name<Ltt> {
        #[inline(always)]
        fn from(_token: #struct_name<Ctt>) -> Self {
          unsafe { Self::new() }
        }
      }

      impl From<#struct_name<Ttt>> for #struct_name<Ltt> {
        #[inline(always)]
        fn from(_token: #struct_name<Ttt>) -> Self {
          unsafe { Self::new() }
        }
      }

      #interrupt
    },
    quote! {
      #field_name: Some(#struct_name::<Ltt>::handler)
    },
    quote! {
      #thread_local::new(#index)
    },
    quote! {
      #(#attrs)*
      pub #field_name: #struct_name<Ctt>
    },
    quote! {
      #field_name: unsafe { #struct_name::new() }
    },
  ))
}
