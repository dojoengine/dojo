"use strict";

var debug = require('debug');

var debugFunc;
var phase = 'default';
var namespace = '';

var newDebug = function newDebug() {
  debugFunc = namespace ? debug("fetch-mock:".concat(phase, ":").concat(namespace)) : debug("fetch-mock:".concat(phase));
};

var newDebugSandbox = function newDebugSandbox(ns) {
  return debug("fetch-mock:".concat(phase, ":").concat(ns));
};

newDebug();
module.exports = {
  debug: function debug() {
    debugFunc.apply(void 0, arguments);
  },
  setDebugNamespace: function setDebugNamespace(str) {
    namespace = str;
    newDebug();
  },
  setDebugPhase: function setDebugPhase(str) {
    phase = str || 'default';
    newDebug();
  },
  getDebug: function getDebug(namespace) {
    return newDebugSandbox(namespace);
  }
};