"use strict";

// avoid circular dependency when using jest.mock()
var fetch;

try {
  // note that jest is not a global, but is injected somehow into
  // the environment. So we can't be safe and check for global.jest
  // Hence the try/catch
  fetch = jest.requireActual('node-fetch'); //eslint-disable-line no-undef
} catch (e) {
  fetch = require('node-fetch');
}

var Request = fetch.Request;
var Response = fetch.Response;
var Headers = fetch.Headers;

var Stream = require('stream');

var FetchMock = require('./lib/index');

var http = require('http');

var _require = require('./lib/request-utils'),
    setUrlImplementation = _require.setUrlImplementation;

setUrlImplementation(require('whatwg-url').URL);
FetchMock.global = global;
FetchMock.statusTextMap = http.STATUS_CODES;
FetchMock.Stream = Stream;
FetchMock.config = Object.assign(FetchMock.config, {
  Promise: Promise,
  Request: Request,
  Response: Response,
  Headers: Headers,
  fetch: fetch
});
module.exports = FetchMock.createInstance();