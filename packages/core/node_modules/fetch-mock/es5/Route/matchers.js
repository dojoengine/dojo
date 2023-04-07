"use strict";

var _interopRequireDefault = require("@babel/runtime/helpers/interopRequireDefault");

var _defineProperty2 = _interopRequireDefault(require("@babel/runtime/helpers/defineProperty"));

var _require = require('../lib/debug'),
    debug = _require.debug;

var _glob = require('glob-to-regexp');

var pathToRegexp = require('path-to-regexp');

var querystring = require('querystring');

var isSubset = require('is-subset');

var _require2 = require('../lib/request-utils'),
    headerUtils = _require2.headers,
    getPath = _require2.getPath,
    getQuery = _require2.getQuery,
    normalizeUrl = _require2.normalizeUrl;

var isEqual = require('lodash.isequal');

var debuggableUrlFunc = function debuggableUrlFunc(func) {
  return function (url) {
    debug('Actual url:', url);
    return func(url);
  };
};

var stringMatchers = {
  begin: function begin(targetString) {
    return debuggableUrlFunc(function (url) {
      return url.indexOf(targetString) === 0;
    });
  },
  end: function end(targetString) {
    return debuggableUrlFunc(function (url) {
      return url.substr(-targetString.length) === targetString;
    });
  },
  glob: function glob(targetString) {
    var urlRX = _glob(targetString);

    return debuggableUrlFunc(function (url) {
      return urlRX.test(url);
    });
  },
  express: function express(targetString) {
    var urlRX = pathToRegexp(targetString);
    return debuggableUrlFunc(function (url) {
      return urlRX.test(getPath(url));
    });
  },
  path: function path(targetString) {
    return debuggableUrlFunc(function (url) {
      return getPath(url) === targetString;
    });
  }
};

var getHeaderMatcher = function getHeaderMatcher(_ref) {
  var expectedHeaders = _ref.headers;
  debug('Generating header matcher');

  if (!expectedHeaders) {
    debug('  No header expectations defined - skipping');
    return;
  }

  var expectation = headerUtils.toLowerCase(expectedHeaders);
  debug('  Expected headers:', expectation);
  return function (url, _ref2) {
    var _ref2$headers = _ref2.headers,
        headers = _ref2$headers === void 0 ? {} : _ref2$headers;
    debug('Attempting to match headers');
    var lowerCaseHeaders = headerUtils.toLowerCase(headerUtils.normalize(headers));
    debug('  Expected headers:', expectation);
    debug('  Actual headers:', lowerCaseHeaders);
    return Object.keys(expectation).every(function (headerName) {
      return headerUtils.equal(lowerCaseHeaders[headerName], expectation[headerName]);
    });
  };
};

var getMethodMatcher = function getMethodMatcher(_ref3) {
  var expectedMethod = _ref3.method;
  debug('Generating method matcher');

  if (!expectedMethod) {
    debug('  No method expectations defined - skipping');
    return;
  }

  debug('  Expected method:', expectedMethod);
  return function (url, _ref4) {
    var method = _ref4.method;
    debug('Attempting to match method');
    var actualMethod = method ? method.toLowerCase() : 'get';
    debug('  Expected method:', expectedMethod);
    debug('  Actual method:', actualMethod);
    return expectedMethod === actualMethod;
  };
};

var getQueryStringMatcher = function getQueryStringMatcher(_ref5) {
  var passedQuery = _ref5.query;
  debug('Generating query parameters matcher');

  if (!passedQuery) {
    debug('  No query parameters expectations defined - skipping');
    return;
  }

  var expectedQuery = querystring.parse(querystring.stringify(passedQuery));
  debug('  Expected query parameters:', passedQuery);
  var keys = Object.keys(expectedQuery);
  return function (url) {
    debug('Attempting to match query parameters');
    var query = querystring.parse(getQuery(url));
    debug('  Expected query parameters:', expectedQuery);
    debug('  Actual query parameters:', query);
    return keys.every(function (key) {
      if (Array.isArray(query[key])) {
        if (!Array.isArray(expectedQuery[key])) {
          return false;
        } else {
          return isEqual(query[key].sort(), expectedQuery[key].sort());
        }
      }

      return query[key] === expectedQuery[key];
    });
  };
};

var getParamsMatcher = function getParamsMatcher(_ref6) {
  var expectedParams = _ref6.params,
      matcherUrl = _ref6.url;
  debug('Generating path parameters matcher');

  if (!expectedParams) {
    debug('  No path parameters expectations defined - skipping');
    return;
  }

  if (!/express:/.test(matcherUrl)) {
    throw new Error('fetch-mock: matching on params is only possible when using an express: matcher');
  }

  debug('  Expected path parameters:', expectedParams);
  var expectedKeys = Object.keys(expectedParams);
  var keys = [];
  var re = pathToRegexp(matcherUrl.replace(/^express:/, ''), keys);
  return function (url) {
    debug('Attempting to match path parameters');
    var vals = re.exec(getPath(url)) || [];
    vals.shift();
    var params = keys.reduce(function (map, _ref7, i) {
      var name = _ref7.name;
      return vals[i] ? Object.assign(map, (0, _defineProperty2["default"])({}, name, vals[i])) : map;
    }, {});
    debug('  Expected path parameters:', expectedParams);
    debug('  Actual path parameters:', params);
    return expectedKeys.every(function (key) {
      return params[key] === expectedParams[key];
    });
  };
};

var getBodyMatcher = function getBodyMatcher(route, fetchMock) {
  var matchPartialBody = fetchMock.getOption('matchPartialBody', route);
  var expectedBody = route.body;
  debug('Generating body matcher');
  return function (url, _ref8) {
    var body = _ref8.body,
        _ref8$method = _ref8.method,
        method = _ref8$method === void 0 ? 'get' : _ref8$method;
    debug('Attempting to match body');

    if (method.toLowerCase() === 'get') {
      debug('  GET request - skip matching body'); // GET requests don’t send a body so the body matcher should be ignored for them

      return true;
    }

    var sentBody;

    try {
      debug('  Parsing request body as JSON');
      sentBody = JSON.parse(body);
    } catch (err) {
      debug('  Failed to parse request body as JSON', err);
    }

    debug('Expected body:', expectedBody);
    debug('Actual body:', sentBody);

    if (matchPartialBody) {
      debug('matchPartialBody is true - checking for partial match only');
    }

    return sentBody && (matchPartialBody ? isSubset(sentBody, expectedBody) : isEqual(sentBody, expectedBody));
  };
};

var getFullUrlMatcher = function getFullUrlMatcher(route, matcherUrl, query) {
  // if none of the special syntaxes apply, it's just a simple string match
  // but we have to be careful to normalize the url we check and the name
  // of the route to allow for e.g. http://it.at.there being indistinguishable
  // from http://it.at.there/ once we start generating Request/Url objects
  debug('  Matching using full url', matcherUrl);
  var expectedUrl = normalizeUrl(matcherUrl);
  debug('  Normalised url to:', matcherUrl);

  if (route.identifier === matcherUrl) {
    debug('  Updating route identifier to match normalized url:', matcherUrl);
    route.identifier = expectedUrl;
  }

  return function (matcherUrl) {
    debug('Expected url:', expectedUrl);
    debug('Actual url:', matcherUrl);

    if (query && expectedUrl.indexOf('?')) {
      debug('Ignoring query string when matching url');
      return matcherUrl.indexOf(expectedUrl) === 0;
    }

    return normalizeUrl(matcherUrl) === expectedUrl;
  };
};

var getFunctionMatcher = function getFunctionMatcher(_ref9) {
  var functionMatcher = _ref9.functionMatcher;
  debug('Detected user defined function matcher', functionMatcher);
  return function () {
    for (var _len = arguments.length, args = new Array(_len), _key = 0; _key < _len; _key++) {
      args[_key] = arguments[_key];
    }

    debug('Calling function matcher with arguments', args);
    return functionMatcher.apply(void 0, args);
  };
};

var getUrlMatcher = function getUrlMatcher(route) {
  debug('Generating url matcher');
  var matcherUrl = route.url,
      query = route.query;

  if (matcherUrl === '*') {
    debug('  Using universal * rule to match any url');
    return function () {
      return true;
    };
  }

  if (matcherUrl instanceof RegExp) {
    debug('  Using regular expression to match url:', matcherUrl);
    return function (url) {
      return matcherUrl.test(url);
    };
  }

  if (matcherUrl.href) {
    debug("  Using URL object to match url", matcherUrl);
    return getFullUrlMatcher(route, matcherUrl.href, query);
  }

  for (var shorthand in stringMatchers) {
    if (matcherUrl.indexOf(shorthand + ':') === 0) {
      debug("  Using ".concat(shorthand, ": pattern to match url"), matcherUrl);
      var urlFragment = matcherUrl.replace(new RegExp("^".concat(shorthand, ":")), '');
      return stringMatchers[shorthand](urlFragment);
    }
  }

  return getFullUrlMatcher(route, matcherUrl, query);
};

module.exports = [{
  name: 'query',
  matcher: getQueryStringMatcher
}, {
  name: 'method',
  matcher: getMethodMatcher
}, {
  name: 'headers',
  matcher: getHeaderMatcher
}, {
  name: 'params',
  matcher: getParamsMatcher
}, {
  name: 'body',
  matcher: getBodyMatcher,
  usesBody: true
}, {
  name: 'functionMatcher',
  matcher: getFunctionMatcher
}, {
  name: 'url',
  matcher: getUrlMatcher
}];