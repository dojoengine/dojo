"use strict";

var _interopRequireDefault = require("@babel/runtime/helpers/interopRequireDefault");

var _slicedToArray2 = _interopRequireDefault(require("@babel/runtime/helpers/slicedToArray"));

var _classCallCheck2 = _interopRequireDefault(require("@babel/runtime/helpers/classCallCheck"));

var _createClass2 = _interopRequireDefault(require("@babel/runtime/helpers/createClass"));

var _typeof2 = _interopRequireDefault(require("@babel/runtime/helpers/typeof"));

var builtInMatchers = require('./matchers');

var _require = require('../lib/debug'),
    debug = _require.debug,
    setDebugNamespace = _require.setDebugNamespace,
    getDebug = _require.getDebug;

var isUrlMatcher = function isUrlMatcher(matcher) {
  return matcher instanceof RegExp || typeof matcher === 'string' || (0, _typeof2["default"])(matcher) === 'object' && 'href' in matcher;
};

var isFunctionMatcher = function isFunctionMatcher(matcher) {
  return typeof matcher === 'function';
};

var Route = /*#__PURE__*/function () {
  function Route(args, fetchMock) {
    (0, _classCallCheck2["default"])(this, Route);
    this.fetchMock = fetchMock;
    var debug = getDebug('compileRoute()');
    debug('Compiling route');
    this.init(args);
    this.sanitize();
    this.validate();
    this.generateMatcher();
    this.limit();
    this.delayResponse();
  }

  (0, _createClass2["default"])(Route, [{
    key: "validate",
    value: function validate() {
      var _this = this;

      if (!('response' in this)) {
        throw new Error('fetch-mock: Each route must define a response');
      }

      if (!Route.registeredMatchers.some(function (_ref) {
        var name = _ref.name;
        return name in _this;
      })) {
        throw new Error("fetch-mock: Each route must specify some criteria for matching calls to fetch. To match all calls use '*'");
      }
    }
  }, {
    key: "init",
    value: function init(args) {
      var _args = (0, _slicedToArray2["default"])(args, 3),
          matcher = _args[0],
          response = _args[1],
          _args$ = _args[2],
          options = _args$ === void 0 ? {} : _args$;

      var routeConfig = {};

      if (isUrlMatcher(matcher) || isFunctionMatcher(matcher)) {
        routeConfig.matcher = matcher;
      } else {
        Object.assign(routeConfig, matcher);
      }

      if (typeof response !== 'undefined') {
        routeConfig.response = response;
      }

      Object.assign(routeConfig, options);
      Object.assign(this, routeConfig);
    }
  }, {
    key: "sanitize",
    value: function sanitize() {
      var debug = getDebug('sanitize()');
      debug('Sanitizing route properties');

      if (this.method) {
        debug("Converting method ".concat(this.method, " to lower case"));
        this.method = this.method.toLowerCase();
      }

      if (isUrlMatcher(this.matcher)) {
        debug('Mock uses a url matcher', this.matcher);
        this.url = this.matcher;
        delete this.matcher;
      }

      this.functionMatcher = this.matcher || this.functionMatcher;
      debug('Setting route.identifier...');
      debug("  route.name is ".concat(this.name));
      debug("  route.url is ".concat(this.url));
      debug("  route.functionMatcher is ".concat(this.functionMatcher));
      this.identifier = this.name || this.url || this.functionMatcher;
      debug("  -> route.identifier set to ".concat(this.identifier));
    }
  }, {
    key: "generateMatcher",
    value: function generateMatcher() {
      var _this2 = this;

      setDebugNamespace('generateMatcher()');
      debug('Compiling matcher for route');
      var activeMatchers = Route.registeredMatchers.map(function (_ref2) {
        var name = _ref2.name,
            matcher = _ref2.matcher,
            usesBody = _ref2.usesBody;
        return _this2[name] && {
          matcher: matcher(_this2, _this2.fetchMock),
          usesBody: usesBody
        };
      }).filter(function (matcher) {
        return Boolean(matcher);
      });
      this.usesBody = activeMatchers.some(function (_ref3) {
        var usesBody = _ref3.usesBody;
        return usesBody;
      });
      debug('Compiled matcher for route');
      setDebugNamespace();

      this.matcher = function (url) {
        var options = arguments.length > 1 && arguments[1] !== undefined ? arguments[1] : {};
        var request = arguments.length > 2 ? arguments[2] : undefined;
        return activeMatchers.every(function (_ref4) {
          var matcher = _ref4.matcher;
          return matcher(url, options, request);
        });
      };
    }
  }, {
    key: "limit",
    value: function limit() {
      var _this3 = this;

      var debug = getDebug('limit()');
      debug('Limiting number of requests to handle by route');

      if (!this.repeat) {
        debug('  No `repeat` value set on route. Will match any number of requests');
        return;
      }

      debug("  Route set to repeat ".concat(this.repeat, " times"));
      var matcher = this.matcher;
      var timesLeft = this.repeat;

      this.matcher = function (url, options) {
        var match = timesLeft && matcher(url, options);

        if (match) {
          timesLeft--;
          return true;
        }
      };

      this.reset = function () {
        return timesLeft = _this3.repeat;
      };
    }
  }, {
    key: "delayResponse",
    value: function delayResponse() {
      var _this4 = this;

      var debug = getDebug('delayResponse()');
      debug("Applying response delay settings");

      if (this.delay) {
        debug("  Wrapping response in delay of ".concat(this.delay, " miliseconds"));
        var response = this.response;

        this.response = function () {
          debug("Delaying response by ".concat(_this4.delay, " miliseconds"));
          return new Promise(function (res) {
            return setTimeout(function () {
              return res(response);
            }, _this4.delay);
          });
        };
      } else {
        debug("  No delay set on route. Will respond 'immediately' (but asynchronously)");
      }
    }
  }], [{
    key: "addMatcher",
    value: function addMatcher(matcher) {
      Route.registeredMatchers.push(matcher);
    }
  }]);
  return Route;
}();

Route.registeredMatchers = [];
builtInMatchers.forEach(Route.addMatcher);
module.exports = Route;