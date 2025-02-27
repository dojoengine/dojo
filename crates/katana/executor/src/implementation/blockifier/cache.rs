use std::str::FromStr;
use std::sync::{Arc, LazyLock};

use blockifier::execution::contract_class::{CompiledClassV1, RunnableCompiledClass};
use katana_cairo::starknet_api::contract_class::SierraVersion;
use katana_primitives::class::{ClassHash, CompiledClass, ContractClass};
use quick_cache::sync::Cache;
use rayon::ThreadPoolBuilder;
use tracing::trace;

use super::utils::to_class;

pub static COMPILED_CLASS_CACHE: LazyLock<ClassCache> =
    LazyLock::new(|| ClassCache::new().unwrap());

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "native")]
    #[error(transparent)]
    FailedToCreateThreadPool(#[from] rayon::ThreadPoolBuildError),
}

#[derive(Debug, Clone)]
pub struct ClassCache {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    #[cfg(feature = "native")]
    pool: rayon::ThreadPool,
    cache: Cache<ClassHash, RunnableCompiledClass>,
}

impl ClassCache {
    pub fn new() -> Result<Self, Error> {
        const CACHE_SIZE: usize = 100;
        let cache = Cache::new(CACHE_SIZE);

        #[cfg(feature = "native")]
        let pool = ThreadPoolBuilder::new()
            .num_threads(3)
            .thread_name(|i| format!("cache-native-compiler-{i}"))
            .build()?;

        Ok(Self {
            inner: Arc::new(Inner {
                cache,
                #[cfg(feature = "native")]
                pool,
            }),
        })
    }

    pub fn get(&self, hash: &ClassHash) -> Option<RunnableCompiledClass> {
        self.inner.cache.get(hash)
    }

    pub fn insert(&self, hash: ClassHash, class: ContractClass) -> RunnableCompiledClass {
        match class {
            ContractClass::Legacy(..) => {
                let class = class.compile().unwrap();
                let class = to_class(class).unwrap();
                self.inner.cache.insert(hash, class.clone());
                class
            }

            ContractClass::Class(ref sierra) => {
                #[cfg(feature = "native")]
                use blockifier::execution::native::contract_class::NativeCompiledClassV1;
                #[cfg(feature = "native")]
                use cairo_native::executor::AotContractExecutor;
                #[cfg(feature = "native")]
                use cairo_native::OptLevel;

                #[cfg(feature = "native")]
                let program = sierra.extract_sierra_program().unwrap();
                #[cfg(feature = "native")]
                let entry_points = sierra.entry_points_by_type.clone();

                let CompiledClass::Class(casm) = class.compile().unwrap() else {
                    unreachable!("cant be legacy")
                };

                let version = SierraVersion::from_str(&casm.compiler_version).unwrap();
                let compiled = CompiledClassV1::try_from((casm, version)).unwrap();

                #[cfg(feature = "native")]
                let inner = self.inner.clone();
                #[cfg(feature = "native")]
                let compiled_clone = compiled.clone();

                #[cfg(feature = "native")]
                self.inner.pool.spawn(move || {
                    trace!(target: "class_cache", class = format!("{hash:#x}"), "Compiling native class");

                    let executor =
                        AotContractExecutor::new(&program, &entry_points, OptLevel::Default)
                            .unwrap();

                    let native = NativeCompiledClassV1::new(executor, compiled_clone);
                    inner.cache.insert(hash, RunnableCompiledClass::V1Native(native));

                    trace!(target: "class_cache", class = format!("{hash:#x}"), "Native class compiled")
                });

                let class = RunnableCompiledClass::V1(compiled);
                self.inner.cache.insert(hash, class.clone());

                class
            }
        }
    }
}
