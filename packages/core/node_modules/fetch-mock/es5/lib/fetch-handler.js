"use strict";

var _interopRequireDefault = require("@babel/runtime/helpers/interopRequireDefault");

var _slicedToArray2 = _interopRequireDefault(require("@babel/runtime/helpers/slicedToArray"));

var _regenerator = _interopRequireDefault(require("@babel/runtime/regenerator"));

var _asyncToGenerator2 = _interopRequireDefault(require("@babel/runtime/helpers/asyncToGenerator"));

var _classCallCheck2 = _interopRequireDefault(require("@babel/runtime/helpers/classCallCheck"));

var _assertThisInitialized2 = _interopRequireDefault(require("@babel/runtime/helpers/assertThisInitialized"));

var _inherits2 = _interopRequireDefault(require("@babel/runtime/helpers/inherits"));

var _possibleConstructorReturn2 = _interopRequireDefault(require("@babel/runtime/helpers/possibleConstructorReturn"));

var _getPrototypeOf2 = _interopRequireDefault(require("@babel/runtime/helpers/getPrototypeOf"));

var _wrapNativeSuper2 = _interopRequireDefault(require("@babel/runtime/helpers/wrapNativeSuper"));

function _createSuper(Derived) { var hasNativeReflectConstruct = _isNativeReflectConstruct(); return function _createSuperInternal() { var Super = (0, _getPrototypeOf2["default"])(Derived), result; if (hasNativeReflectConstruct) { var NewTarget = (0, _getPrototypeOf2["default"])(this).constructor; result = Reflect.construct(Super, arguments, NewTarget); } else { result = Super.apply(this, arguments); } return (0, _possibleConstructorReturn2["default"])(this, result); }; }

function _isNativeReflectConstruct() { if (typeof Reflect === "undefined" || !Reflect.construct) return false; if (Reflect.construct.sham) return false; if (typeof Proxy === "function") return true; try { Date.prototype.toString.call(Reflect.construct(Date, [], function () {})); return true; } catch (e) { return false; } }

var _require = require('./debug'),
    debug = _require.debug,
    setDebugPhase = _require.setDebugPhase,
    getDebug = _require.getDebug;

var responseBuilder = require('./response-builder');

var requestUtils = require('./request-utils');

var FetchMock = {}; // see https://heycam.github.io/webidl/#aborterror for the standardised interface
// Note that this differs slightly from node-fetch

var AbortError = /*#__PURE__*/function (_Error) {
  (0, _inherits2["default"])(AbortError, _Error);

  var _super = _createSuper(AbortError);

  function AbortError() {
    var _this;

    (0, _classCallCheck2["default"])(this, AbortError);
    _this = _super.apply(this, arguments);
    _this.name = 'AbortError';
    _this.message = 'The operation was aborted.'; // Do not include this class in the stacktrace

    if (Error.captureStackTrace) {
      Error.captureStackTrace((0, _assertThisInitialized2["default"])(_this), _this.constructor);
    }

    return _this;
  }

  return AbortError;
}( /*#__PURE__*/(0, _wrapNativeSuper2["default"])(Error)); // Patch native fetch to avoid "NotSupportedError:ReadableStream uploading is not supported" in Safari.
// See also https://github.com/wheresrhys/fetch-mock/issues/584
// See also https://stackoverflow.com/a/50952018/1273406


var patchNativeFetchForSafari = function patchNativeFetchForSafari(nativeFetch) {
  // Try to patch fetch only on Safari
  if (typeof navigator === 'undefined' || !navigator.vendor || navigator.vendor !== 'Apple Computer, Inc.') {
    return nativeFetch;
  } // It seems the code is working on Safari thus patch native fetch to avoid the error.


  return /*#__PURE__*/function () {
    var _ref = (0, _asyncToGenerator2["default"])( /*#__PURE__*/_regenerator["default"].mark(function _callee(request) {
      var method, body, cache, credentials, headers, integrity, mode, redirect, referrer, init;
      return _regenerator["default"].wrap(function _callee$(_context) {
        while (1) {
          switch (_context.prev = _context.next) {
            case 0:
              method = request.method;

              if (['POST', 'PUT', 'PATCH'].includes(method)) {
                _context.next = 3;
                break;
              }

              return _context.abrupt("return", nativeFetch(request));

            case 3:
              _context.next = 5;
              return request.clone().text();

            case 5:
              body = _context.sent;
              cache = request.cache, credentials = request.credentials, headers = request.headers, integrity = request.integrity, mode = request.mode, redirect = request.redirect, referrer = request.referrer;
              init = {
                body: body,
                cache: cache,
                credentials: credentials,
                headers: headers,
                integrity: integrity,
                mode: mode,
                redirect: redirect,
                referrer: referrer,
                method: method
              };
              return _context.abrupt("return", nativeFetch(request.url, init));

            case 9:
            case "end":
              return _context.stop();
          }
        }
      }, _callee);
    }));

    return function (_x) {
      return _ref.apply(this, arguments);
    };
  }();
};

var resolve = /*#__PURE__*/function () {
  var _ref2 = (0, _asyncToGenerator2["default"])( /*#__PURE__*/_regenerator["default"].mark(function _callee2(_ref3, url, options, request) {
    var response, _ref3$responseIsFetch, responseIsFetch, debug;

    return _regenerator["default"].wrap(function _callee2$(_context2) {
      while (1) {
        switch (_context2.prev = _context2.next) {
          case 0:
            response = _ref3.response, _ref3$responseIsFetch = _ref3.responseIsFetch, responseIsFetch = _ref3$responseIsFetch === void 0 ? false : _ref3$responseIsFetch;
            debug = getDebug('resolve()');
            debug('Recursively resolving function and promise responses'); // We want to allow things like
            // - function returning a Promise for a response
            // - delaying (using a timeout Promise) a function's execution to generate
            //   a response
            // Because of this we can't safely check for function before Promisey-ness,
            // or vice versa. So to keep it DRY, and flexible, we keep trying until we
            // have something that looks like neither Promise nor function

          case 3:
            if (!true) {
              _context2.next = 31;
              break;
            }

            if (!(typeof response === 'function')) {
              _context2.next = 18;
              break;
            }

            debug('  Response is a function'); // in the case of falling back to the network we need to make sure we're using
            // the original Request instance, not our normalised url + options

            if (!responseIsFetch) {
              _context2.next = 14;
              break;
            }

            if (!request) {
              _context2.next = 10;
              break;
            }

            debug('  -> Calling fetch with Request instance');
            return _context2.abrupt("return", response(request));

          case 10:
            debug('  -> Calling fetch with url and options');
            return _context2.abrupt("return", response(url, options));

          case 14:
            debug('  -> Calling response function');
            response = response(url, options, request);

          case 16:
            _context2.next = 29;
            break;

          case 18:
            if (!(typeof response.then === 'function')) {
              _context2.next = 26;
              break;
            }

            debug('  Response is a promise');
            debug('  -> Resolving promise');
            _context2.next = 23;
            return response;

          case 23:
            response = _context2.sent;
            _context2.next = 29;
            break;

          case 26:
            debug('  Response is not a function or a promise');
            debug('  -> Exiting response resolution recursion');
            return _context2.abrupt("return", response);

          case 29:
            _context2.next = 3;
            break;

          case 31:
          case "end":
            return _context2.stop();
        }
      }
    }, _callee2);
  }));

  return function resolve(_x2, _x3, _x4, _x5) {
    return _ref2.apply(this, arguments);
  };
}();

FetchMock.needsAsyncBodyExtraction = function (_ref4) {
  var request = _ref4.request;
  return request && this.routes.some(function (_ref5) {
    var usesBody = _ref5.usesBody;
    return usesBody;
  });
};

FetchMock.fetchHandler = function (url, options) {
  setDebugPhase('handle');
  var debug = getDebug('fetchHandler()');
  debug('fetch called with:', url, options);
  var normalizedRequest = requestUtils.normalizeRequest(url, options, this.config.Request);
  debug('Request normalised');
  debug('  url', normalizedRequest.url);
  debug('  options', normalizedRequest.options);
  debug('  request', normalizedRequest.request);
  debug('  signal', normalizedRequest.signal);

  if (this.needsAsyncBodyExtraction(normalizedRequest)) {
    debug('Need to wait for Body to be streamed before calling router: switching to async mode');
    return this._extractBodyThenHandle(normalizedRequest);
  }

  return this._fetchHandler(normalizedRequest);
};

FetchMock._extractBodyThenHandle = /*#__PURE__*/function () {
  var _ref6 = (0, _asyncToGenerator2["default"])( /*#__PURE__*/_regenerator["default"].mark(function _callee3(normalizedRequest) {
    return _regenerator["default"].wrap(function _callee3$(_context3) {
      while (1) {
        switch (_context3.prev = _context3.next) {
          case 0:
            _context3.next = 2;
            return normalizedRequest.options.body;

          case 2:
            normalizedRequest.options.body = _context3.sent;
            return _context3.abrupt("return", this._fetchHandler(normalizedRequest));

          case 4:
          case "end":
            return _context3.stop();
        }
      }
    }, _callee3, this);
  }));

  return function (_x6) {
    return _ref6.apply(this, arguments);
  };
}();

FetchMock._fetchHandler = function (_ref7) {
  var _this2 = this;

  var url = _ref7.url,
      options = _ref7.options,
      request = _ref7.request,
      signal = _ref7.signal;

  var _this$executeRouter = this.executeRouter(url, options, request),
      route = _this$executeRouter.route,
      callLog = _this$executeRouter.callLog;

  this.recordCall(callLog); // this is used to power the .flush() method

  var done;

  this._holdingPromises.push(new this.config.Promise(function (res) {
    return done = res;
  })); // wrapped in this promise to make sure we respect custom Promise
  // constructors defined by the user


  return new this.config.Promise(function (res, rej) {
    if (signal) {
      debug('signal exists - enabling fetch abort');

      var abort = function abort() {
        debug('aborting fetch'); // note that DOMException is not available in node.js;
        // even node-fetch uses a custom error class:
        // https://github.com/bitinn/node-fetch/blob/master/src/abort-error.js

        rej(typeof DOMException !== 'undefined' ? new DOMException('The operation was aborted.', 'AbortError') : new AbortError());
        done();
      };

      if (signal.aborted) {
        debug('signal is already aborted - aborting the fetch');
        abort();
      }

      signal.addEventListener('abort', abort);
    }

    _this2.generateResponse({
      route: route,
      url: url,
      options: options,
      request: request,
      callLog: callLog
    }).then(res, rej).then(done, done).then(function () {
      setDebugPhase();
    });
  });
};

FetchMock.fetchHandler.isMock = true;

FetchMock.executeRouter = function (url, options, request) {
  var debug = getDebug('executeRouter()');
  var callLog = {
    url: url,
    options: options,
    request: request,
    isUnmatched: true
  };
  debug("Attempting to match request to a route");

  if (this.getOption('fallbackToNetwork') === 'always') {
    debug('  Configured with fallbackToNetwork=always - passing through to fetch');
    return {
      route: {
        response: this.getNativeFetch(),
        responseIsFetch: true
      } // BUG - this callLog never used to get sent. Discovered the bug
      // but can't fix outside a major release as it will potentially
      // cause too much disruption
      //
      // callLog,

    };
  }

  var route = this.router(url, options, request);

  if (route) {
    debug('  Matching route found');
    return {
      route: route,
      callLog: {
        url: url,
        options: options,
        request: request,
        identifier: route.identifier
      }
    };
  }

  if (this.getOption('warnOnFallback')) {
    console.warn("Unmatched ".concat(options && options.method || 'GET', " to ").concat(url)); // eslint-disable-line
  }

  if (this.fallbackResponse) {
    debug('  No matching route found - using fallbackResponse');
    return {
      route: {
        response: this.fallbackResponse
      },
      callLog: callLog
    };
  }

  if (!this.getOption('fallbackToNetwork')) {
    throw new Error("fetch-mock: No fallback response defined for ".concat(options && options.method || 'GET', " to ").concat(url));
  }

  debug('  Configured to fallbackToNetwork - passing through to fetch');
  return {
    route: {
      response: this.getNativeFetch(),
      responseIsFetch: true
    },
    callLog: callLog
  };
};

FetchMock.generateResponse = /*#__PURE__*/function () {
  var _ref8 = (0, _asyncToGenerator2["default"])( /*#__PURE__*/_regenerator["default"].mark(function _callee4(_ref9) {
    var route, url, options, request, _ref9$callLog, callLog, debug, response, _responseBuilder, _responseBuilder2, realResponse, finalResponse;

    return _regenerator["default"].wrap(function _callee4$(_context4) {
      while (1) {
        switch (_context4.prev = _context4.next) {
          case 0:
            route = _ref9.route, url = _ref9.url, options = _ref9.options, request = _ref9.request, _ref9$callLog = _ref9.callLog, callLog = _ref9$callLog === void 0 ? {} : _ref9$callLog;
            debug = getDebug('generateResponse()');
            _context4.next = 4;
            return resolve(route, url, options, request);

          case 4:
            response = _context4.sent;

            if (!(response["throws"] && typeof response !== 'function')) {
              _context4.next = 8;
              break;
            }

            debug('response.throws is defined - throwing an error');
            throw response["throws"];

          case 8:
            if (!this.config.Response.prototype.isPrototypeOf(response)) {
              _context4.next = 12;
              break;
            }

            debug('response is already a Response instance - returning it');
            callLog.response = response;
            return _context4.abrupt("return", response);

          case 12:
            // finally, if we need to convert config into a response, we do it
            _responseBuilder = responseBuilder({
              url: url,
              responseConfig: response,
              fetchMock: this,
              route: route
            }), _responseBuilder2 = (0, _slicedToArray2["default"])(_responseBuilder, 2), realResponse = _responseBuilder2[0], finalResponse = _responseBuilder2[1];
            callLog.response = realResponse;
            return _context4.abrupt("return", finalResponse);

          case 15:
          case "end":
            return _context4.stop();
        }
      }
    }, _callee4, this);
  }));

  return function (_x7) {
    return _ref8.apply(this, arguments);
  };
}();

FetchMock.router = function (url, options, request) {
  var route = this.routes.find(function (route, i) {
    debug("Trying to match route ".concat(i));
    return route.matcher(url, options, request);
  });

  if (route) {
    return route;
  }
};

FetchMock.getNativeFetch = function () {
  var func = this.realFetch || this.isSandbox && this.config.fetch;

  if (!func) {
    throw new Error('fetch-mock: Falling back to network only available on global fetch-mock, or by setting config.fetch on sandboxed fetch-mock');
  }

  return patchNativeFetchForSafari(func);
};

FetchMock.recordCall = function (obj) {
  debug('Recording fetch call', obj);

  if (obj) {
    this._calls.push(obj);
  }
};

module.exports = FetchMock;