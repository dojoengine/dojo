trait IMiddleware {
  fn before_execute(&self, ctx: Context) -> Context;
  fn after_execute(&self, ctx: Context) -> Context; 
}

impl IMiddleware for Middleware {

  fn before_execute(&self, ctx: Context) -> Context {
    // execute before core
    return ctx;
  }

  fn after_execute(&self, ctx: Context) -> Context {
    // execute after code
    return ctx; 
  }
}