pub mod source;
pub mod storage;
pub mod types;

// pub struct World<S, R> {
//     address: FieldElement,
//     state: S,
//     source: Option<R>,
// }

// impl<S, R> World<S, R>
// where
//     S: Storage + StorageReader,
//     R: Source,
// {
//     pub fn new(address: FieldElement, state: S) -> Self {
//         Self { address, state, source: None }
//     }

//     pub fn load_from_source(&mut self, source: R) -> Result<(), R::Error> {
//         source.load(self.address, &mut self.state)?;
//         self.source = Some(source);
//         Ok(())
//     }

//     pub fn address(&self) -> FieldElement {
//         self.address
//     }
// }

// impl<S, R> StorageReader for World<S, R>
// where
//     S: StorageReader,
//     R: Source,
// {
//     type Error = S::Error;

//     fn component(&self, component: String) -> Result<Component, Self::Error> {
//         self.state.component(component)
//     }

//     fn system(&self, system: String) -> Result<System, Self::Error> {
//         self.state.system(system)
//     }

//     fn entities(
//         &self,
//         component: String,
//         partition: starknet::core::types::FieldElement,
//     ) -> Result<Vec<Vec<starknet::core::types::FieldElement>>, Self::Error> {
//         self.state.entities(component, partition)
//     }

//     fn entity(
//         &self,
//         component: String,
//         partition: starknet::core::types::FieldElement,
//         key: starknet::core::types::FieldElement,
//     ) -> Result<Vec<starknet::core::types::FieldElement>, Self::Error> {
//         self.state.entity(component, partition, key)
//     }

//     fn executor(&self) -> Result<starknet::core::types::FieldElement, Self::Error> {
//         self.state.executor()
//     }
// }
