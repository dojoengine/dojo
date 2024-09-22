// macro usage example

// // user can set the individual fields when using the macro
// #[katana_runner::test(
// 	block_time = 1000,
//     fee = false,
//     accounts = 1,
//     chain_id = 0x12345,
//     validation = false,
//     db_dir = "path/to/db",
//     binary = "path/to/binary",
//     output = "path/to/output.log",
//     genesis = "path/to/genesis.json",
//     classes = ["path/to/erc20.json", "path/to/account.json"] // will be declared at startup
// )]
// async fn foo(node: &mut RunnerCtx) -> Result<(), Box<dyn std::error::Error>> {
//     let url = node.url();
//     let provider = node.provider();
//     let account = node.account();
//     Ok(())
// }

// // but they could also just pass the whole RunnerCfg struct if they want to allow defining the
// // configuration once and reuse it across different tests

// #[katana_runner::test(RunnerCfg { })]
// async fn foo(node: &mut RunnerCtx) -> Result<(), Box<dyn std::error::Error>> {
//     let url = node.url();
//     let provider = node.provider();
//     let account = node.account();
//     Ok(())
// }

// // This is the rough expanded code of the proc macro

// #[tokio::test]
// async fn foo() -> Result<(), Box<dyn std::error::Error>> {
//     struct RunnerCtx(KatanaRunnerHandle);

//     impl std::ops::Deref for RunnerCtx {
//         type Target = KatanaRunnerHandle;
//         fn deref(&self) -> &Self::Target {
//             &self.0
//         }
//     }
//     impl std::ops::DerefMut for RunnerCtx {
//         fn deref_mut(&mut self) -> &mut Self::Target {
//             &mut self.0
//         }
//     }

//     let handle = KatanaRunner::todo();

//     async fn __foo(node: &mut RunnerCtx) -> Result<(), Box<dyn std::error::Error>> {
//         Ok(())
//     }

//     __foo(&mut RunnerCtx(handle)).await
// }
