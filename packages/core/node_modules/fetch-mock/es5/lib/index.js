"use strict";

var _require = require('./debug'),
    debug = _require.debug;

var setUpAndTearDown = require('./set-up-and-tear-down');

var fetchHandler = require('./fetch-handler');

var inspecting = require('./inspecting');

var Route = require('../Route');

var FetchMock = Object.assign({}, fetchHandler, setUpAndTearDown, inspecting);

FetchMock.addMatcher = function (matcher) {
  Route.addMatcher(matcher);
};

FetchMock.config = {
  fallbackToNetwork: false,
  includeContentLength: true,
  sendAsJson: true,
  warnOnFallback: true,
  overwriteRoutes: undefined
};

FetchMock.createInstance = function () {
  var _this = this;

  debug('Creating fetch-mock instance');
  var instance = Object.create(FetchMock);
  instance._uncompiledRoutes = (this._uncompiledRoutes || []).slice();
  instance.routes = instance._uncompiledRoutes.map(function (config) {
    return _this.compileRoute(config);
  });
  instance.fallbackResponse = this.fallbackResponse || undefined;
  instance.config = Object.assign({}, this.config || FetchMock.config);
  instance._calls = [];
  instance._holdingPromises = [];
  instance.bindMethods();
  return instance;
};

FetchMock.compileRoute = function (config) {
  return new Route(config, this);
};

FetchMock.bindMethods = function () {
  this.fetchHandler = FetchMock.fetchHandler.bind(this);
  this.reset = this.restore = FetchMock.reset.bind(this);
  this.resetHistory = FetchMock.resetHistory.bind(this);
  this.resetBehavior = FetchMock.resetBehavior.bind(this);
};

FetchMock.sandbox = function () {
  debug('Creating sandboxed fetch-mock instance'); // this construct allows us to create a fetch-mock instance which is also
  // a callable function, while circumventing circularity when defining the
  // object that this function should be bound to

  var fetchMockProxy = function fetchMockProxy(url, options) {
    return sandbox.fetchHandler(url, options);
  };

  var sandbox = Object.assign(fetchMockProxy, // Ensures that the entire returned object is a callable function
  FetchMock, // prototype methods
  this.createInstance(), // instance data
  {
    Headers: this.config.Headers,
    Request: this.config.Request,
    Response: this.config.Response
  });
  sandbox.bindMethods();
  sandbox.isSandbox = true;
  sandbox["default"] = sandbox;
  return sandbox;
};

FetchMock.getOption = function (name) {
  var route = arguments.length > 1 && arguments[1] !== undefined ? arguments[1] : {};
  return name in route ? route[name] : this.config[name];
};

module.exports = FetchMock;