"use strict";

var _interopRequireDefault = require("@babel/runtime/helpers/interopRequireDefault");

var _typeof2 = _interopRequireDefault(require("@babel/runtime/helpers/typeof"));

var _classCallCheck2 = _interopRequireDefault(require("@babel/runtime/helpers/classCallCheck"));

var _createClass2 = _interopRequireDefault(require("@babel/runtime/helpers/createClass"));

var _require = require('./debug'),
    getDebug = _require.getDebug;

var responseConfigProps = ['body', 'headers', 'throws', 'status', 'redirectUrl'];

var ResponseBuilder = /*#__PURE__*/function () {
  function ResponseBuilder(options) {
    (0, _classCallCheck2["default"])(this, ResponseBuilder);
    this.debug = getDebug('ResponseBuilder()');
    this.debug('Response builder created with options', options);
    Object.assign(this, options);
  }

  (0, _createClass2["default"])(ResponseBuilder, [{
    key: "exec",
    value: function exec() {
      this.debug('building response');
      this.normalizeResponseConfig();
      this.constructFetchOpts();
      this.constructResponseBody();
      var realResponse = new this.fetchMock.config.Response(this.body, this.options);
      var proxyResponse = this.buildObservableResponse(realResponse);
      return [realResponse, proxyResponse];
    }
  }, {
    key: "sendAsObject",
    value: function sendAsObject() {
      var _this = this;

      if (responseConfigProps.some(function (prop) {
        return _this.responseConfig[prop];
      })) {
        if (Object.keys(this.responseConfig).every(function (key) {
          return responseConfigProps.includes(key);
        })) {
          return false;
        } else {
          return true;
        }
      } else {
        return true;
      }
    }
  }, {
    key: "normalizeResponseConfig",
    value: function normalizeResponseConfig() {
      // If the response config looks like a status, start to generate a simple response
      if (typeof this.responseConfig === 'number') {
        this.debug('building response using status', this.responseConfig);
        this.responseConfig = {
          status: this.responseConfig
        }; // If the response config is not an object, or is an object that doesn't use
        // any reserved properties, assume it is meant to be the body of the response
      } else if (typeof this.responseConfig === 'string' || this.sendAsObject()) {
        this.debug('building text response from', this.responseConfig);
        this.responseConfig = {
          body: this.responseConfig
        };
      }
    }
  }, {
    key: "validateStatus",
    value: function validateStatus(status) {
      if (!status) {
        this.debug('No status provided. Defaulting to 200');
        return 200;
      }

      if (typeof status === 'number' && parseInt(status, 10) !== status && status >= 200 || status < 600) {
        this.debug('Valid status provided', status);
        return status;
      }

      throw new TypeError("fetch-mock: Invalid status ".concat(status, " passed on response object.\nTo respond with a JSON object that has status as a property assign the object to body\ne.g. {\"body\": {\"status: \"registered\"}}"));
    }
  }, {
    key: "constructFetchOpts",
    value: function constructFetchOpts() {
      this.options = this.responseConfig.options || {};
      this.options.url = this.responseConfig.redirectUrl || this.url;
      this.options.status = this.validateStatus(this.responseConfig.status);
      this.options.statusText = this.fetchMock.statusTextMap[String(this.options.status)]; // Set up response headers. The empty object is to cope with
      // new Headers(undefined) throwing in Chrome
      // https://code.google.com/p/chromium/issues/detail?id=335871

      this.options.headers = new this.fetchMock.config.Headers(this.responseConfig.headers || {});
    }
  }, {
    key: "getOption",
    value: function getOption(name) {
      return this.fetchMock.getOption(name, this.route);
    }
  }, {
    key: "convertToJson",
    value: function convertToJson() {
      // convert to json if we need to
      if (this.getOption('sendAsJson') && this.responseConfig.body != null && //eslint-disable-line
      (0, _typeof2["default"])(this.body) === 'object') {
        this.debug('Stringifying JSON response body');
        this.body = JSON.stringify(this.body);

        if (!this.options.headers.has('Content-Type')) {
          this.options.headers.set('Content-Type', 'application/json');
        }
      }
    }
  }, {
    key: "setContentLength",
    value: function setContentLength() {
      // add a Content-Length header if we need to
      if (this.getOption('includeContentLength') && typeof this.body === 'string' && !this.options.headers.has('Content-Length')) {
        this.debug('Setting content-length header:', this.body.length.toString());
        this.options.headers.set('Content-Length', this.body.length.toString());
      }
    }
  }, {
    key: "constructResponseBody",
    value: function constructResponseBody() {
      // start to construct the body
      this.body = this.responseConfig.body;
      this.convertToJson();
      this.setContentLength(); // On the server we need to manually construct the readable stream for the
      // Response object (on the client this done automatically)

      if (this.Stream) {
        this.debug('Creating response stream');
        var stream = new this.Stream.Readable();

        if (this.body != null) {
          //eslint-disable-line
          stream.push(this.body, 'utf-8');
        }

        stream.push(null);
        this.body = stream;
      }

      this.body = this.body;
    }
  }, {
    key: "buildObservableResponse",
    value: function buildObservableResponse(response) {
      var _this2 = this;

      var fetchMock = this.fetchMock;
      response._fmResults = {}; // Using a proxy means we can set properties that may not be writable on
      // the original Response. It also means we can track the resolution of
      // promises returned by res.json(), res.text() etc

      this.debug('Wrapping Response in ES proxy for observability');
      return new Proxy(response, {
        get: function get(originalResponse, name) {
          if (_this2.responseConfig.redirectUrl) {
            if (name === 'url') {
              _this2.debug('Retrieving redirect url', _this2.responseConfig.redirectUrl);

              return _this2.responseConfig.redirectUrl;
            }

            if (name === 'redirected') {
              _this2.debug('Retrieving redirected status', true);

              return true;
            }
          }

          if (typeof originalResponse[name] === 'function') {
            _this2.debug('Wrapping body promises in ES proxies for observability');

            return new Proxy(originalResponse[name], {
              apply: function apply(func, thisArg, args) {
                _this2.debug("Calling res.".concat(name));

                var result = func.apply(response, args);

                if (result.then) {
                  fetchMock._holdingPromises.push(result["catch"](function () {
                    return null;
                  }));

                  originalResponse._fmResults[name] = result;
                }

                return result;
              }
            });
          }

          return originalResponse[name];
        }
      });
    }
  }]);
  return ResponseBuilder;
}();

module.exports = function (options) {
  return new ResponseBuilder(options).exec();
};