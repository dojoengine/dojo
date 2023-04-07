"use strict";

var FetchMock = require('./lib/index');

var statusTextMap = require('./lib/status-text');

var theGlobal = typeof window !== 'undefined' ? window : self;

var _require = require('./lib/request-utils'),
    setUrlImplementation = _require.setUrlImplementation;

setUrlImplementation(theGlobal.URL);
FetchMock.global = theGlobal;
FetchMock.statusTextMap = statusTextMap;
FetchMock.config = Object.assign(FetchMock.config, {
  Promise: theGlobal.Promise,
  Request: theGlobal.Request,
  Response: theGlobal.Response,
  Headers: theGlobal.Headers
});
module.exports = FetchMock.createInstance();