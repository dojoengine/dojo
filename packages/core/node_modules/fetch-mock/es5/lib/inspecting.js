"use strict";

var _interopRequireDefault = require("@babel/runtime/helpers/interopRequireDefault");

var _regenerator = _interopRequireDefault(require("@babel/runtime/regenerator"));

var _asyncToGenerator2 = _interopRequireDefault(require("@babel/runtime/helpers/asyncToGenerator"));

var _slicedToArray2 = _interopRequireDefault(require("@babel/runtime/helpers/slicedToArray"));

var _toConsumableArray2 = _interopRequireDefault(require("@babel/runtime/helpers/toConsumableArray"));

var _require = require('./debug'),
    setDebugPhase = _require.setDebugPhase,
    setDebugNamespace = _require.setDebugNamespace,
    debug = _require.debug;

var _require2 = require('./request-utils'),
    normalizeUrl = _require2.normalizeUrl;

var Route = require('../Route');

var FetchMock = {};

var isName = function isName(nameOrMatcher) {
  return typeof nameOrMatcher === 'string' && /^[\da-zA-Z\-]+$/.test(nameOrMatcher);
};

var filterCallsWithMatcher = function filterCallsWithMatcher(matcher) {
  var options = arguments.length > 1 && arguments[1] !== undefined ? arguments[1] : {};
  var calls = arguments.length > 2 ? arguments[2] : undefined;

  var _Route = new Route([Object.assign({
    matcher: matcher,
    response: 'ok'
  }, options)], this);

  matcher = _Route.matcher;
  return calls.filter(function (_ref) {
    var url = _ref.url,
        options = _ref.options;
    return matcher(normalizeUrl(url), options);
  });
};

var formatDebug = function formatDebug(func) {
  return function () {
    setDebugPhase('inspect');

    for (var _len = arguments.length, args = new Array(_len), _key = 0; _key < _len; _key++) {
      args[_key] = arguments[_key];
    }

    var result = func.call.apply(func, [this].concat(args));
    setDebugPhase();
    return result;
  };
};

var callObjToArray = function callObjToArray(obj) {
  if (!obj) {
    return undefined;
  }

  var url = obj.url,
      options = obj.options,
      request = obj.request,
      identifier = obj.identifier,
      isUnmatched = obj.isUnmatched,
      response = obj.response;
  var arr = [url, options];
  arr.request = request;
  arr.identifier = identifier;
  arr.isUnmatched = isUnmatched;
  arr.response = response;
  return arr;
};

FetchMock.filterCalls = function (nameOrMatcher, options) {
  debug('Filtering fetch calls');
  var calls = this._calls;
  var matcher = '*';

  if ([true, 'matched'].includes(nameOrMatcher)) {
    debug("Filter provided is ".concat(nameOrMatcher, ". Returning matched calls only"));
    calls = calls.filter(function (_ref2) {
      var isUnmatched = _ref2.isUnmatched;
      return !isUnmatched;
    });
  } else if ([false, 'unmatched'].includes(nameOrMatcher)) {
    debug("Filter provided is ".concat(nameOrMatcher, ". Returning unmatched calls only"));
    calls = calls.filter(function (_ref3) {
      var isUnmatched = _ref3.isUnmatched;
      return isUnmatched;
    });
  } else if (typeof nameOrMatcher === 'undefined') {
    debug("Filter provided is undefined. Returning all calls");
    calls = calls;
  } else if (isName(nameOrMatcher)) {
    debug("Filter provided, looks like the name of a named route. Returning only calls handled by that route");
    calls = calls.filter(function (_ref4) {
      var identifier = _ref4.identifier;
      return identifier === nameOrMatcher;
    });
  } else {
    matcher = nameOrMatcher === '*' ? '*' : normalizeUrl(nameOrMatcher);

    if (this.routes.some(function (_ref5) {
      var identifier = _ref5.identifier;
      return identifier === matcher;
    })) {
      debug("Filter provided, ".concat(nameOrMatcher, ", identifies a route. Returning only calls handled by that route"));
      calls = calls.filter(function (call) {
        return call.identifier === matcher;
      });
    }
  }

  if ((options || matcher !== '*') && calls.length) {
    if (typeof options === 'string') {
      options = {
        method: options
      };
    }

    debug('Compiling filter and options to route in order to filter all calls', nameOrMatcher);
    calls = filterCallsWithMatcher.call(this, matcher, options, calls);
  }

  debug("Retrieved ".concat(calls.length, " calls"));
  return calls.map(callObjToArray);
};

FetchMock.calls = formatDebug(function (nameOrMatcher, options) {
  debug('retrieving matching calls');
  return this.filterCalls(nameOrMatcher, options);
});
FetchMock.lastCall = formatDebug(function (nameOrMatcher, options) {
  debug('retrieving last matching call');
  return (0, _toConsumableArray2["default"])(this.filterCalls(nameOrMatcher, options)).pop();
});
FetchMock.lastUrl = formatDebug(function (nameOrMatcher, options) {
  debug('retrieving url of last matching call');
  return (this.lastCall(nameOrMatcher, options) || [])[0];
});
FetchMock.lastOptions = formatDebug(function (nameOrMatcher, options) {
  debug('retrieving options of last matching call');
  return (this.lastCall(nameOrMatcher, options) || [])[1];
});
FetchMock.lastResponse = formatDebug(function (nameOrMatcher, options) {
  debug('retrieving respose of last matching call');
  console.warn("When doing all the following:\n- using node-fetch\n- responding with a real network response (using spy() or fallbackToNetwork)\n- using `fetchMock.LastResponse()`\n- awaiting the body content\n... the response will hang unless your source code also awaits the response body.\nThis is an unavoidable consequence of the nodejs implementation of streams.\n");
  var response = (this.lastCall(nameOrMatcher, options) || []).response;

  try {
    var clonedResponse = response.clone();
    return clonedResponse;
  } catch (err) {
    Object.entries(response._fmResults).forEach(function (_ref6) {
      var _ref7 = (0, _slicedToArray2["default"])(_ref6, 2),
          name = _ref7[0],
          result = _ref7[1];

      response[name] = function () {
        return result;
      };
    });
    return response;
  }
});
FetchMock.called = formatDebug(function (nameOrMatcher, options) {
  debug('checking if matching call was made');
  return Boolean(this.filterCalls(nameOrMatcher, options).length);
});
FetchMock.flush = formatDebug( /*#__PURE__*/function () {
  var _ref8 = (0, _asyncToGenerator2["default"])( /*#__PURE__*/_regenerator["default"].mark(function _callee(waitForResponseMethods) {
    var queuedPromises;
    return _regenerator["default"].wrap(function _callee$(_context) {
      while (1) {
        switch (_context.prev = _context.next) {
          case 0:
            setDebugNamespace('flush');
            debug("flushing all fetch calls. ".concat(waitForResponseMethods ? '' : 'Not ', "waiting for response bodies to complete download"));
            queuedPromises = this._holdingPromises;
            this._holdingPromises = [];
            debug("".concat(queuedPromises.length, " fetch calls to be awaited"));
            _context.next = 7;
            return Promise.all(queuedPromises);

          case 7:
            debug("All fetch calls have completed");

            if (!(waitForResponseMethods && this._holdingPromises.length)) {
              _context.next = 13;
              break;
            }

            debug("Awaiting all fetch bodies to download");
            _context.next = 12;
            return this.flush(waitForResponseMethods);

          case 12:
            debug("All fetch bodies have completed downloading");

          case 13:
            setDebugNamespace();

          case 14:
          case "end":
            return _context.stop();
        }
      }
    }, _callee, this);
  }));

  return function (_x) {
    return _ref8.apply(this, arguments);
  };
}());
FetchMock.done = formatDebug(function (nameOrMatcher) {
  var _this = this;

  setDebugPhase('inspect');
  setDebugNamespace('done');
  debug('Checking to see if expected calls have been made');
  var routesToCheck;

  if (nameOrMatcher && typeof nameOrMatcher !== 'boolean') {
    debug('Checking to see if expected calls have been made for single route:', nameOrMatcher);
    routesToCheck = [{
      identifier: nameOrMatcher
    }];
  } else {
    debug('Checking to see if expected calls have been made for all routes');
    routesToCheck = this.routes;
  } // Can't use array.every because would exit after first failure, which would
  // break the logging


  var result = routesToCheck.map(function (_ref9) {
    var identifier = _ref9.identifier;

    if (!_this.called(identifier)) {
      debug('No calls made for route:', identifier);
      console.warn("Warning: ".concat(identifier, " not called")); // eslint-disable-line

      return false;
    }

    var expectedTimes = (_this.routes.find(function (r) {
      return r.identifier === identifier;
    }) || {}).repeat;

    if (!expectedTimes) {
      debug('Route has been called at least once, and no expectation of more set:', identifier);
      return true;
    }

    var actualTimes = _this.filterCalls(identifier).length;

    debug("Route called ".concat(actualTimes, " times:"), identifier);

    if (expectedTimes > actualTimes) {
      debug("Route called ".concat(actualTimes, " times, but expected ").concat(expectedTimes, ":"), identifier);
      console.warn("Warning: ".concat(identifier, " only called ").concat(actualTimes, " times, but ").concat(expectedTimes, " expected")); // eslint-disable-line

      return false;
    } else {
      return true;
    }
  }).every(function (isDone) {
    return isDone;
  });
  setDebugNamespace();
  setDebugPhase();
  return result;
});
module.exports = FetchMock;