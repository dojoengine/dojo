"use strict";

var _interopRequireDefault = require("@babel/runtime/helpers/interopRequireDefault");

var _typeof2 = _interopRequireDefault(require("@babel/runtime/helpers/typeof"));

var _regenerator = _interopRequireDefault(require("@babel/runtime/regenerator"));

var _asyncToGenerator2 = _interopRequireDefault(require("@babel/runtime/helpers/asyncToGenerator"));

var _defineProperty2 = _interopRequireDefault(require("@babel/runtime/helpers/defineProperty"));

var _slicedToArray2 = _interopRequireDefault(require("@babel/runtime/helpers/slicedToArray"));

var _toConsumableArray2 = _interopRequireDefault(require("@babel/runtime/helpers/toConsumableArray"));

var URL; // https://stackoverflow.com/a/19709846/308237
// split, URL constructor does not support protocol-relative urls

var absoluteUrlRX = new RegExp('^[a-z]+://', 'i');
var protocolRelativeUrlRX = new RegExp('^//', 'i');

var headersToArray = function headersToArray(headers) {
  // node-fetch 1 Headers
  if (typeof headers.raw === 'function') {
    return Object.entries(headers.raw());
  } else if (headers[Symbol.iterator]) {
    return (0, _toConsumableArray2["default"])(headers);
  } else {
    return Object.entries(headers);
  }
};

var zipObject = function zipObject(entries) {
  return entries.reduce(function (obj, _ref) {
    var _ref2 = (0, _slicedToArray2["default"])(_ref, 2),
        key = _ref2[0],
        val = _ref2[1];

    return Object.assign(obj, (0, _defineProperty2["default"])({}, key, val));
  }, {});
};

var normalizeUrl = function normalizeUrl(url) {
  if (typeof url === 'function' || url instanceof RegExp || /^(begin|end|glob|express|path)\:/.test(url)) {
    return url;
  }

  if (absoluteUrlRX.test(url)) {
    var u = new URL(url);
    return u.href;
  } else if (protocolRelativeUrlRX.test(url)) {
    var _u = new URL(url, 'http://dummy');

    return _u.href;
  } else {
    var _u2 = new URL(url, 'http://dummy');

    return _u2.pathname + _u2.search;
  }
};

var extractBody = /*#__PURE__*/function () {
  var _ref3 = (0, _asyncToGenerator2["default"])( /*#__PURE__*/_regenerator["default"].mark(function _callee(request) {
    return _regenerator["default"].wrap(function _callee$(_context) {
      while (1) {
        switch (_context.prev = _context.next) {
          case 0:
            _context.prev = 0;

            if (!('body' in request)) {
              _context.next = 3;
              break;
            }

            return _context.abrupt("return", request.body.toString());

          case 3:
            return _context.abrupt("return", request.clone().text());

          case 6:
            _context.prev = 6;
            _context.t0 = _context["catch"](0);

          case 8:
          case "end":
            return _context.stop();
        }
      }
    }, _callee, null, [[0, 6]]);
  }));

  return function extractBody(_x) {
    return _ref3.apply(this, arguments);
  };
}();

module.exports = {
  setUrlImplementation: function setUrlImplementation(it) {
    URL = it;
  },
  normalizeRequest: function normalizeRequest(url, options, Request) {
    if (Request.prototype.isPrototypeOf(url)) {
      var derivedOptions = {
        method: url.method
      };
      var body = extractBody(url);

      if (typeof body !== 'undefined') {
        derivedOptions.body = body;
      }

      var normalizedRequestObject = {
        url: normalizeUrl(url.url),
        options: Object.assign(derivedOptions, options),
        request: url,
        signal: options && options.signal || url.signal
      };
      var headers = headersToArray(url.headers);

      if (headers.length) {
        normalizedRequestObject.options.headers = zipObject(headers);
      }

      return normalizedRequestObject;
    } else if (typeof url === 'string' || // horrible URL object duck-typing
    (0, _typeof2["default"])(url) === 'object' && 'href' in url) {
      return {
        url: normalizeUrl(url),
        options: options,
        signal: options && options.signal
      };
    } else if ((0, _typeof2["default"])(url) === 'object') {
      throw new TypeError('fetch-mock: Unrecognised Request object. Read the Config and Installation sections of the docs');
    } else {
      throw new TypeError('fetch-mock: Invalid arguments passed to fetch');
    }
  },
  normalizeUrl: normalizeUrl,
  getPath: function getPath(url) {
    var u = absoluteUrlRX.test(url) ? new URL(url) : new URL(url, 'http://dummy');
    return u.pathname;
  },
  getQuery: function getQuery(url) {
    var u = absoluteUrlRX.test(url) ? new URL(url) : new URL(url, 'http://dummy');
    return u.search ? u.search.substr(1) : '';
  },
  headers: {
    normalize: function normalize(headers) {
      return zipObject(headersToArray(headers));
    },
    toLowerCase: function toLowerCase(headers) {
      return Object.keys(headers).reduce(function (obj, k) {
        obj[k.toLowerCase()] = headers[k];
        return obj;
      }, {});
    },
    equal: function equal(actualHeader, expectedHeader) {
      actualHeader = Array.isArray(actualHeader) ? actualHeader : [actualHeader];
      expectedHeader = Array.isArray(expectedHeader) ? expectedHeader : [expectedHeader];

      if (actualHeader.length !== expectedHeader.length) {
        return false;
      }

      return actualHeader.every(function (val, i) {
        return val === expectedHeader[i];
      });
    }
  }
};