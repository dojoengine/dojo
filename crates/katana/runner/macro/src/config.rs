use std::path::PathBuf;

pub const DEFAULT_ERROR_CONFIG: Configuration = Configuration::new(true);

pub struct Configuration {
    dev: bool,
    fee: bool,
    is_test: bool,
    accounts: u16,
    validation: bool,
    db_dir: Option<PathBuf>,
    binary: Option<String>,
    block_time: Option<u64>,
    log_path: Option<PathBuf>,
}

impl Configuration {
    const fn new(is_test: bool) -> Self {
        Configuration {
            is_test,
            fee: true,
            db_dir: None,
            binary: None,
            accounts: 10,
            dev: is_test,
            log_path: None,
            validation: true,
            block_time: None,
        }
    }

    // fn set_flavor(&mut self, runtime: syn::Lit, span: Span) -> Result<(), syn::Error> {
    //     if self.flavor.is_some() {
    //         return Err(syn::Error::new(span, "`flavor` set multiple times."));
    //     }

    //     let runtime_str = parse_string(runtime, span, "flavor")?;
    //     let runtime =
    //         RuntimeFlavor::from_str(&runtime_str).map_err(|err| syn::Error::new(span, err))?;
    //     self.flavor = Some(runtime);
    //     Ok(())
    // }

    // fn set_worker_threads(
    //     &mut self,
    //     worker_threads: syn::Lit,
    //     span: Span,
    // ) -> Result<(), syn::Error> {
    //     if self.worker_threads.is_some() {
    //         return Err(syn::Error::new(span, "`worker_threads` set multiple times."));
    //     }

    //     let worker_threads = parse_int(worker_threads, span, "worker_threads")?;
    //     if worker_threads == 0 {
    //         return Err(syn::Error::new(span, "`worker_threads` may not be 0."));
    //     }
    //     self.worker_threads = Some((worker_threads, span));
    //     Ok(())
    // }

    // fn set_start_paused(&mut self, start_paused: syn::Lit, span: Span) -> Result<(), syn::Error>
    // {     if self.start_paused.is_some() {
    //         return Err(syn::Error::new(span, "`start_paused` set multiple times."));
    //     }

    //     let start_paused = parse_bool(start_paused, span, "start_paused")?;
    //     self.start_paused = Some((start_paused, span));
    //     Ok(())
    // }

    // fn set_crate_name(&mut self, name: syn::Lit, span: Span) -> Result<(), syn::Error> {
    //     if self.crate_name.is_some() {
    //         return Err(syn::Error::new(span, "`crate` set multiple times."));
    //     }
    //     let name_path = parse_path(name, span, "crate")?;
    //     self.crate_name = Some(name_path);
    //     Ok(())
    // }

    // fn set_unhandled_panic(
    //     &mut self,
    //     unhandled_panic: syn::Lit,
    //     span: Span,
    // ) -> Result<(), syn::Error> {
    //     if self.unhandled_panic.is_some() {
    //         return Err(syn::Error::new(span, "`unhandled_panic` set multiple times."));
    //     }

    //     let unhandled_panic = parse_string(unhandled_panic, span, "unhandled_panic")?;
    //     let unhandled_panic =
    //         UnhandledPanic::from_str(&unhandled_panic).map_err(|err| syn::Error::new(span,
    // err))?;     self.unhandled_panic = Some((unhandled_panic, span));
    //     Ok(())
    // }

    // fn macro_name(&self) -> &'static str {
    //     if self.is_test {
    //         "tokio::test"
    //     } else {
    //         "tokio::main"
    //     }
    // }

    // fn build(&self) -> Result<FinalConfig, syn::Error> {
    //     use RuntimeFlavor as F;

    //     let flavor = self.flavor.unwrap_or(self.default_flavor);
    //     let worker_threads = match (flavor, self.worker_threads) {
    //         (F::CurrentThread, Some((_, worker_threads_span))) => {
    //             let msg = format!(
    //                 "The `worker_threads` option requires the `multi_thread` runtime flavor. Use
    // `#[{}(flavor = \"multi_thread\")]`",                 self.macro_name(),
    //             );
    //             return Err(syn::Error::new(worker_threads_span, msg));
    //         }
    //         (F::CurrentThread, None) => None,
    //         (F::Threaded, worker_threads) if self.rt_multi_thread_available => {
    //             worker_threads.map(|(val, _span)| val)
    //         }
    //         (F::Threaded, _) => {
    //             let msg = if self.flavor.is_none() {
    //                 "The default runtime flavor is `multi_thread`, but the `rt-multi-thread`
    // feature is disabled."             } else {
    //                 "The runtime flavor `multi_thread` requires the `rt-multi-thread` feature."
    //             };
    //             return Err(syn::Error::new(Span::call_site(), msg));
    //         }
    //     };

    //     let start_paused = match (flavor, self.start_paused) {
    //         (F::Threaded, Some((_, start_paused_span))) => {
    //             let msg = format!(
    //                 "The `start_paused` option requires the `current_thread` runtime flavor. Use
    // `#[{}(flavor = \"current_thread\")]`",                 self.macro_name(),
    //             );
    //             return Err(syn::Error::new(start_paused_span, msg));
    //         }
    //         (F::CurrentThread, Some((start_paused, _))) => Some(start_paused),
    //         (_, None) => None,
    //     };

    //     let unhandled_panic = match (flavor, self.unhandled_panic) {
    //         (F::Threaded, Some((_, unhandled_panic_span))) => {
    //             let msg = format!(
    //                 "The `unhandled_panic` option requires the `current_thread` runtime flavor.
    // Use `#[{}(flavor = \"current_thread\")]`",                 self.macro_name(),
    //             );
    //             return Err(syn::Error::new(unhandled_panic_span, msg));
    //         }
    //         (F::CurrentThread, Some((unhandled_panic, _))) => Some(unhandled_panic),
    //         (_, None) => None,
    //     };

    //     Ok(FinalConfig {
    //         crate_name: self.crate_name.clone(),
    //         flavor,
    //         worker_threads,
    //         start_paused,
    //         unhandled_panic,
    //     })
    // }
}
