(function (global, factory) {
            typeof exports === 'object' && typeof module !== 'undefined' ? module.exports = factory() :
            typeof define === 'function' && define.amd ? define(factory) :
            (global = global || self, global.fetchMock = factory());
}(this, (function () { 'use strict';

            var global$1 = (typeof global !== "undefined" ? global :
                        typeof self !== "undefined" ? self :
                        typeof window !== "undefined" ? window : {});

            // shim for using process in browser
            // based off https://github.com/defunctzombie/node-process/blob/master/browser.js

            function defaultSetTimout() {
                throw new Error('setTimeout has not been defined');
            }
            function defaultClearTimeout () {
                throw new Error('clearTimeout has not been defined');
            }
            var cachedSetTimeout = defaultSetTimout;
            var cachedClearTimeout = defaultClearTimeout;
            if (typeof global$1.setTimeout === 'function') {
                cachedSetTimeout = setTimeout;
            }
            if (typeof global$1.clearTimeout === 'function') {
                cachedClearTimeout = clearTimeout;
            }

            function runTimeout(fun) {
                if (cachedSetTimeout === setTimeout) {
                    //normal enviroments in sane situations
                    return setTimeout(fun, 0);
                }
                // if setTimeout wasn't available but was latter defined
                if ((cachedSetTimeout === defaultSetTimout || !cachedSetTimeout) && setTimeout) {
                    cachedSetTimeout = setTimeout;
                    return setTimeout(fun, 0);
                }
                try {
                    // when when somebody has screwed with setTimeout but no I.E. maddness
                    return cachedSetTimeout(fun, 0);
                } catch(e){
                    try {
                        // When we are in I.E. but the script has been evaled so I.E. doesn't trust the global object when called normally
                        return cachedSetTimeout.call(null, fun, 0);
                    } catch(e){
                        // same as above but when it's a version of I.E. that must have the global object for 'this', hopfully our context correct otherwise it will throw a global error
                        return cachedSetTimeout.call(this, fun, 0);
                    }
                }


            }
            function runClearTimeout(marker) {
                if (cachedClearTimeout === clearTimeout) {
                    //normal enviroments in sane situations
                    return clearTimeout(marker);
                }
                // if clearTimeout wasn't available but was latter defined
                if ((cachedClearTimeout === defaultClearTimeout || !cachedClearTimeout) && clearTimeout) {
                    cachedClearTimeout = clearTimeout;
                    return clearTimeout(marker);
                }
                try {
                    // when when somebody has screwed with setTimeout but no I.E. maddness
                    return cachedClearTimeout(marker);
                } catch (e){
                    try {
                        // When we are in I.E. but the script has been evaled so I.E. doesn't  trust the global object when called normally
                        return cachedClearTimeout.call(null, marker);
                    } catch (e){
                        // same as above but when it's a version of I.E. that must have the global object for 'this', hopfully our context correct otherwise it will throw a global error.
                        // Some versions of I.E. have different rules for clearTimeout vs setTimeout
                        return cachedClearTimeout.call(this, marker);
                    }
                }



            }
            var queue = [];
            var draining = false;
            var currentQueue;
            var queueIndex = -1;

            function cleanUpNextTick() {
                if (!draining || !currentQueue) {
                    return;
                }
                draining = false;
                if (currentQueue.length) {
                    queue = currentQueue.concat(queue);
                } else {
                    queueIndex = -1;
                }
                if (queue.length) {
                    drainQueue();
                }
            }

            function drainQueue() {
                if (draining) {
                    return;
                }
                var timeout = runTimeout(cleanUpNextTick);
                draining = true;

                var len = queue.length;
                while(len) {
                    currentQueue = queue;
                    queue = [];
                    while (++queueIndex < len) {
                        if (currentQueue) {
                            currentQueue[queueIndex].run();
                        }
                    }
                    queueIndex = -1;
                    len = queue.length;
                }
                currentQueue = null;
                draining = false;
                runClearTimeout(timeout);
            }
            function nextTick(fun) {
                var args = new Array(arguments.length - 1);
                if (arguments.length > 1) {
                    for (var i = 1; i < arguments.length; i++) {
                        args[i - 1] = arguments[i];
                    }
                }
                queue.push(new Item(fun, args));
                if (queue.length === 1 && !draining) {
                    runTimeout(drainQueue);
                }
            }
            // v8 likes predictible objects
            function Item(fun, array) {
                this.fun = fun;
                this.array = array;
            }
            Item.prototype.run = function () {
                this.fun.apply(null, this.array);
            };
            var title = 'browser';
            var platform = 'browser';
            var browser = true;
            var env = {};
            var argv = [];
            var version = ''; // empty string to avoid regexp issues
            var versions = {};
            var release = {};
            var config = {};

            function noop() {}

            var on = noop;
            var addListener = noop;
            var once = noop;
            var off = noop;
            var removeListener = noop;
            var removeAllListeners = noop;
            var emit = noop;

            function binding(name) {
                throw new Error('process.binding is not supported');
            }

            function cwd () { return '/' }
            function chdir (dir) {
                throw new Error('process.chdir is not supported');
            }function umask() { return 0; }

            // from https://github.com/kumavis/browser-process-hrtime/blob/master/index.js
            var performance = global$1.performance || {};
            var performanceNow =
              performance.now        ||
              performance.mozNow     ||
              performance.msNow      ||
              performance.oNow       ||
              performance.webkitNow  ||
              function(){ return (new Date()).getTime() };

            // generate timestamp or delta
            // see http://nodejs.org/api/process.html#process_process_hrtime
            function hrtime(previousTimestamp){
              var clocktime = performanceNow.call(performance)*1e-3;
              var seconds = Math.floor(clocktime);
              var nanoseconds = Math.floor((clocktime%1)*1e9);
              if (previousTimestamp) {
                seconds = seconds - previousTimestamp[0];
                nanoseconds = nanoseconds - previousTimestamp[1];
                if (nanoseconds<0) {
                  seconds--;
                  nanoseconds += 1e9;
                }
              }
              return [seconds,nanoseconds]
            }

            var startTime = new Date();
            function uptime() {
              var currentTime = new Date();
              var dif = currentTime - startTime;
              return dif / 1000;
            }

            var process = {
              nextTick: nextTick,
              title: title,
              browser: browser,
              env: env,
              argv: argv,
              version: version,
              versions: versions,
              on: on,
              addListener: addListener,
              once: once,
              off: off,
              removeListener: removeListener,
              removeAllListeners: removeAllListeners,
              emit: emit,
              binding: binding,
              cwd: cwd,
              chdir: chdir,
              umask: umask,
              hrtime: hrtime,
              platform: platform,
              release: release,
              config: config,
              uptime: uptime
            };

            var commonjsGlobal = typeof globalThis !== 'undefined' ? globalThis : typeof window !== 'undefined' ? window : typeof global !== 'undefined' ? global : typeof self !== 'undefined' ? self : {};

            function unwrapExports (x) {
            	return x && x.__esModule && Object.prototype.hasOwnProperty.call(x, 'default') ? x['default'] : x;
            }

            function createCommonjsModule(fn, module) {
            	return module = { exports: {} }, fn(module, module.exports), module.exports;
            }

            /**
             * Helpers.
             */

            var s = 1000;
            var m = s * 60;
            var h = m * 60;
            var d = h * 24;
            var w = d * 7;
            var y = d * 365.25;

            /**
             * Parse or format the given `val`.
             *
             * Options:
             *
             *  - `long` verbose formatting [false]
             *
             * @param {String|Number} val
             * @param {Object} [options]
             * @throws {Error} throw an error if val is not a non-empty string or a number
             * @return {String|Number}
             * @api public
             */

            var ms = function(val, options) {
              options = options || {};
              var type = typeof val;
              if (type === 'string' && val.length > 0) {
                return parse(val);
              } else if (type === 'number' && isFinite(val)) {
                return options.long ? fmtLong(val) : fmtShort(val);
              }
              throw new Error(
                'val is not a non-empty string or a valid number. val=' +
                  JSON.stringify(val)
              );
            };

            /**
             * Parse the given `str` and return milliseconds.
             *
             * @param {String} str
             * @return {Number}
             * @api private
             */

            function parse(str) {
              str = String(str);
              if (str.length > 100) {
                return;
              }
              var match = /^(-?(?:\d+)?\.?\d+) *(milliseconds?|msecs?|ms|seconds?|secs?|s|minutes?|mins?|m|hours?|hrs?|h|days?|d|weeks?|w|years?|yrs?|y)?$/i.exec(
                str
              );
              if (!match) {
                return;
              }
              var n = parseFloat(match[1]);
              var type = (match[2] || 'ms').toLowerCase();
              switch (type) {
                case 'years':
                case 'year':
                case 'yrs':
                case 'yr':
                case 'y':
                  return n * y;
                case 'weeks':
                case 'week':
                case 'w':
                  return n * w;
                case 'days':
                case 'day':
                case 'd':
                  return n * d;
                case 'hours':
                case 'hour':
                case 'hrs':
                case 'hr':
                case 'h':
                  return n * h;
                case 'minutes':
                case 'minute':
                case 'mins':
                case 'min':
                case 'm':
                  return n * m;
                case 'seconds':
                case 'second':
                case 'secs':
                case 'sec':
                case 's':
                  return n * s;
                case 'milliseconds':
                case 'millisecond':
                case 'msecs':
                case 'msec':
                case 'ms':
                  return n;
                default:
                  return undefined;
              }
            }

            /**
             * Short format for `ms`.
             *
             * @param {Number} ms
             * @return {String}
             * @api private
             */

            function fmtShort(ms) {
              var msAbs = Math.abs(ms);
              if (msAbs >= d) {
                return Math.round(ms / d) + 'd';
              }
              if (msAbs >= h) {
                return Math.round(ms / h) + 'h';
              }
              if (msAbs >= m) {
                return Math.round(ms / m) + 'm';
              }
              if (msAbs >= s) {
                return Math.round(ms / s) + 's';
              }
              return ms + 'ms';
            }

            /**
             * Long format for `ms`.
             *
             * @param {Number} ms
             * @return {String}
             * @api private
             */

            function fmtLong(ms) {
              var msAbs = Math.abs(ms);
              if (msAbs >= d) {
                return plural(ms, msAbs, d, 'day');
              }
              if (msAbs >= h) {
                return plural(ms, msAbs, h, 'hour');
              }
              if (msAbs >= m) {
                return plural(ms, msAbs, m, 'minute');
              }
              if (msAbs >= s) {
                return plural(ms, msAbs, s, 'second');
              }
              return ms + ' ms';
            }

            /**
             * Pluralization helper.
             */

            function plural(ms, msAbs, n, name) {
              var isPlural = msAbs >= n * 1.5;
              return Math.round(ms / n) + ' ' + name + (isPlural ? 's' : '');
            }

            /**
             * This is the common logic for both the Node.js and web browser
             * implementations of `debug()`.
             */

            function setup(env) {
            	createDebug.debug = createDebug;
            	createDebug.default = createDebug;
            	createDebug.coerce = coerce;
            	createDebug.disable = disable;
            	createDebug.enable = enable;
            	createDebug.enabled = enabled;
            	createDebug.humanize = ms;
            	createDebug.destroy = destroy;

            	Object.keys(env).forEach(key => {
            		createDebug[key] = env[key];
            	});

            	/**
            	* The currently active debug mode names, and names to skip.
            	*/

            	createDebug.names = [];
            	createDebug.skips = [];

            	/**
            	* Map of special "%n" handling functions, for the debug "format" argument.
            	*
            	* Valid key names are a single, lower or upper-case letter, i.e. "n" and "N".
            	*/
            	createDebug.formatters = {};

            	/**
            	* Selects a color for a debug namespace
            	* @param {String} namespace The namespace string for the for the debug instance to be colored
            	* @return {Number|String} An ANSI color code for the given namespace
            	* @api private
            	*/
            	function selectColor(namespace) {
            		let hash = 0;

            		for (let i = 0; i < namespace.length; i++) {
            			hash = ((hash << 5) - hash) + namespace.charCodeAt(i);
            			hash |= 0; // Convert to 32bit integer
            		}

            		return createDebug.colors[Math.abs(hash) % createDebug.colors.length];
            	}
            	createDebug.selectColor = selectColor;

            	/**
            	* Create a debugger with the given `namespace`.
            	*
            	* @param {String} namespace
            	* @return {Function}
            	* @api public
            	*/
            	function createDebug(namespace) {
            		let prevTime;
            		let enableOverride = null;

            		function debug(...args) {
            			// Disabled?
            			if (!debug.enabled) {
            				return;
            			}

            			const self = debug;

            			// Set `diff` timestamp
            			const curr = Number(new Date());
            			const ms = curr - (prevTime || curr);
            			self.diff = ms;
            			self.prev = prevTime;
            			self.curr = curr;
            			prevTime = curr;

            			args[0] = createDebug.coerce(args[0]);

            			if (typeof args[0] !== 'string') {
            				// Anything else let's inspect with %O
            				args.unshift('%O');
            			}

            			// Apply any `formatters` transformations
            			let index = 0;
            			args[0] = args[0].replace(/%([a-zA-Z%])/g, (match, format) => {
            				// If we encounter an escaped % then don't increase the array index
            				if (match === '%%') {
            					return '%';
            				}
            				index++;
            				const formatter = createDebug.formatters[format];
            				if (typeof formatter === 'function') {
            					const val = args[index];
            					match = formatter.call(self, val);

            					// Now we need to remove `args[index]` since it's inlined in the `format`
            					args.splice(index, 1);
            					index--;
            				}
            				return match;
            			});

            			// Apply env-specific formatting (colors, etc.)
            			createDebug.formatArgs.call(self, args);

            			const logFn = self.log || createDebug.log;
            			logFn.apply(self, args);
            		}

            		debug.namespace = namespace;
            		debug.useColors = createDebug.useColors();
            		debug.color = createDebug.selectColor(namespace);
            		debug.extend = extend;
            		debug.destroy = createDebug.destroy; // XXX Temporary. Will be removed in the next major release.

            		Object.defineProperty(debug, 'enabled', {
            			enumerable: true,
            			configurable: false,
            			get: () => enableOverride === null ? createDebug.enabled(namespace) : enableOverride,
            			set: v => {
            				enableOverride = v;
            			}
            		});

            		// Env-specific initialization logic for debug instances
            		if (typeof createDebug.init === 'function') {
            			createDebug.init(debug);
            		}

            		return debug;
            	}

            	function extend(namespace, delimiter) {
            		const newDebug = createDebug(this.namespace + (typeof delimiter === 'undefined' ? ':' : delimiter) + namespace);
            		newDebug.log = this.log;
            		return newDebug;
            	}

            	/**
            	* Enables a debug mode by namespaces. This can include modes
            	* separated by a colon and wildcards.
            	*
            	* @param {String} namespaces
            	* @api public
            	*/
            	function enable(namespaces) {
            		createDebug.save(namespaces);

            		createDebug.names = [];
            		createDebug.skips = [];

            		let i;
            		const split = (typeof namespaces === 'string' ? namespaces : '').split(/[\s,]+/);
            		const len = split.length;

            		for (i = 0; i < len; i++) {
            			if (!split[i]) {
            				// ignore empty strings
            				continue;
            			}

            			namespaces = split[i].replace(/\*/g, '.*?');

            			if (namespaces[0] === '-') {
            				createDebug.skips.push(new RegExp('^' + namespaces.substr(1) + '$'));
            			} else {
            				createDebug.names.push(new RegExp('^' + namespaces + '$'));
            			}
            		}
            	}

            	/**
            	* Disable debug output.
            	*
            	* @return {String} namespaces
            	* @api public
            	*/
            	function disable() {
            		const namespaces = [
            			...createDebug.names.map(toNamespace),
            			...createDebug.skips.map(toNamespace).map(namespace => '-' + namespace)
            		].join(',');
            		createDebug.enable('');
            		return namespaces;
            	}

            	/**
            	* Returns true if the given mode name is enabled, false otherwise.
            	*
            	* @param {String} name
            	* @return {Boolean}
            	* @api public
            	*/
            	function enabled(name) {
            		if (name[name.length - 1] === '*') {
            			return true;
            		}

            		let i;
            		let len;

            		for (i = 0, len = createDebug.skips.length; i < len; i++) {
            			if (createDebug.skips[i].test(name)) {
            				return false;
            			}
            		}

            		for (i = 0, len = createDebug.names.length; i < len; i++) {
            			if (createDebug.names[i].test(name)) {
            				return true;
            			}
            		}

            		return false;
            	}

            	/**
            	* Convert regexp to namespace
            	*
            	* @param {RegExp} regxep
            	* @return {String} namespace
            	* @api private
            	*/
            	function toNamespace(regexp) {
            		return regexp.toString()
            			.substring(2, regexp.toString().length - 2)
            			.replace(/\.\*\?$/, '*');
            	}

            	/**
            	* Coerce `val`.
            	*
            	* @param {Mixed} val
            	* @return {Mixed}
            	* @api private
            	*/
            	function coerce(val) {
            		if (val instanceof Error) {
            			return val.stack || val.message;
            		}
            		return val;
            	}

            	/**
            	* XXX DO NOT USE. This is a temporary stub function.
            	* XXX It WILL be removed in the next major release.
            	*/
            	function destroy() {
            		console.warn('Instance method `debug.destroy()` is deprecated and no longer does anything. It will be removed in the next major version of `debug`.');
            	}

            	createDebug.enable(createDebug.load());

            	return createDebug;
            }

            var common = setup;

            var browser$1 = createCommonjsModule(function (module, exports) {
            /* eslint-env browser */

            /**
             * This is the web browser implementation of `debug()`.
             */

            exports.formatArgs = formatArgs;
            exports.save = save;
            exports.load = load;
            exports.useColors = useColors;
            exports.storage = localstorage();
            exports.destroy = (() => {
            	let warned = false;

            	return () => {
            		if (!warned) {
            			warned = true;
            			console.warn('Instance method `debug.destroy()` is deprecated and no longer does anything. It will be removed in the next major version of `debug`.');
            		}
            	};
            })();

            /**
             * Colors.
             */

            exports.colors = [
            	'#0000CC',
            	'#0000FF',
            	'#0033CC',
            	'#0033FF',
            	'#0066CC',
            	'#0066FF',
            	'#0099CC',
            	'#0099FF',
            	'#00CC00',
            	'#00CC33',
            	'#00CC66',
            	'#00CC99',
            	'#00CCCC',
            	'#00CCFF',
            	'#3300CC',
            	'#3300FF',
            	'#3333CC',
            	'#3333FF',
            	'#3366CC',
            	'#3366FF',
            	'#3399CC',
            	'#3399FF',
            	'#33CC00',
            	'#33CC33',
            	'#33CC66',
            	'#33CC99',
            	'#33CCCC',
            	'#33CCFF',
            	'#6600CC',
            	'#6600FF',
            	'#6633CC',
            	'#6633FF',
            	'#66CC00',
            	'#66CC33',
            	'#9900CC',
            	'#9900FF',
            	'#9933CC',
            	'#9933FF',
            	'#99CC00',
            	'#99CC33',
            	'#CC0000',
            	'#CC0033',
            	'#CC0066',
            	'#CC0099',
            	'#CC00CC',
            	'#CC00FF',
            	'#CC3300',
            	'#CC3333',
            	'#CC3366',
            	'#CC3399',
            	'#CC33CC',
            	'#CC33FF',
            	'#CC6600',
            	'#CC6633',
            	'#CC9900',
            	'#CC9933',
            	'#CCCC00',
            	'#CCCC33',
            	'#FF0000',
            	'#FF0033',
            	'#FF0066',
            	'#FF0099',
            	'#FF00CC',
            	'#FF00FF',
            	'#FF3300',
            	'#FF3333',
            	'#FF3366',
            	'#FF3399',
            	'#FF33CC',
            	'#FF33FF',
            	'#FF6600',
            	'#FF6633',
            	'#FF9900',
            	'#FF9933',
            	'#FFCC00',
            	'#FFCC33'
            ];

            /**
             * Currently only WebKit-based Web Inspectors, Firefox >= v31,
             * and the Firebug extension (any Firefox version) are known
             * to support "%c" CSS customizations.
             *
             * TODO: add a `localStorage` variable to explicitly enable/disable colors
             */

            // eslint-disable-next-line complexity
            function useColors() {
            	// NB: In an Electron preload script, document will be defined but not fully
            	// initialized. Since we know we're in Chrome, we'll just detect this case
            	// explicitly
            	if (typeof window !== 'undefined' && window.process && (window.process.type === 'renderer' || window.process.__nwjs)) {
            		return true;
            	}

            	// Internet Explorer and Edge do not support colors.
            	if (typeof navigator !== 'undefined' && navigator.userAgent && navigator.userAgent.toLowerCase().match(/(edge|trident)\/(\d+)/)) {
            		return false;
            	}

            	// Is webkit? http://stackoverflow.com/a/16459606/376773
            	// document is undefined in react-native: https://github.com/facebook/react-native/pull/1632
            	return (typeof document !== 'undefined' && document.documentElement && document.documentElement.style && document.documentElement.style.WebkitAppearance) ||
            		// Is firebug? http://stackoverflow.com/a/398120/376773
            		(typeof window !== 'undefined' && window.console && (window.console.firebug || (window.console.exception && window.console.table))) ||
            		// Is firefox >= v31?
            		// https://developer.mozilla.org/en-US/docs/Tools/Web_Console#Styling_messages
            		(typeof navigator !== 'undefined' && navigator.userAgent && navigator.userAgent.toLowerCase().match(/firefox\/(\d+)/) && parseInt(RegExp.$1, 10) >= 31) ||
            		// Double check webkit in userAgent just in case we are in a worker
            		(typeof navigator !== 'undefined' && navigator.userAgent && navigator.userAgent.toLowerCase().match(/applewebkit\/(\d+)/));
            }

            /**
             * Colorize log arguments if enabled.
             *
             * @api public
             */

            function formatArgs(args) {
            	args[0] = (this.useColors ? '%c' : '') +
            		this.namespace +
            		(this.useColors ? ' %c' : ' ') +
            		args[0] +
            		(this.useColors ? '%c ' : ' ') +
            		'+' + module.exports.humanize(this.diff);

            	if (!this.useColors) {
            		return;
            	}

            	const c = 'color: ' + this.color;
            	args.splice(1, 0, c, 'color: inherit');

            	// The final "%c" is somewhat tricky, because there could be other
            	// arguments passed either before or after the %c, so we need to
            	// figure out the correct index to insert the CSS into
            	let index = 0;
            	let lastC = 0;
            	args[0].replace(/%[a-zA-Z%]/g, match => {
            		if (match === '%%') {
            			return;
            		}
            		index++;
            		if (match === '%c') {
            			// We only are interested in the *last* %c
            			// (the user may have provided their own)
            			lastC = index;
            		}
            	});

            	args.splice(lastC, 0, c);
            }

            /**
             * Invokes `console.debug()` when available.
             * No-op when `console.debug` is not a "function".
             * If `console.debug` is not available, falls back
             * to `console.log`.
             *
             * @api public
             */
            exports.log = console.debug || console.log || (() => {});

            /**
             * Save `namespaces`.
             *
             * @param {String} namespaces
             * @api private
             */
            function save(namespaces) {
            	try {
            		if (namespaces) {
            			exports.storage.setItem('debug', namespaces);
            		} else {
            			exports.storage.removeItem('debug');
            		}
            	} catch (error) {
            		// Swallow
            		// XXX (@Qix-) should we be logging these?
            	}
            }

            /**
             * Load `namespaces`.
             *
             * @return {String} returns the previously persisted debug modes
             * @api private
             */
            function load() {
            	let r;
            	try {
            		r = exports.storage.getItem('debug');
            	} catch (error) {
            		// Swallow
            		// XXX (@Qix-) should we be logging these?
            	}

            	// If debug isn't set in LS, and we're in Electron, try to load $DEBUG
            	if (!r && typeof process !== 'undefined' && 'env' in process) {
            		r = process.env.DEBUG;
            	}

            	return r;
            }

            /**
             * Localstorage attempts to return the localstorage.
             *
             * This is necessary because safari throws
             * when a user disables cookies/localstorage
             * and you attempt to access it.
             *
             * @return {LocalStorage}
             * @api private
             */

            function localstorage() {
            	try {
            		// TVMLKit (Apple TV JS Runtime) does not have a window object, just localStorage in the global context
            		// The Browser also has localStorage in the global context.
            		return localStorage;
            	} catch (error) {
            		// Swallow
            		// XXX (@Qix-) should we be logging these?
            	}
            }

            module.exports = common(exports);

            const {formatters} = module.exports;

            /**
             * Map %j to `JSON.stringify()`, since no Web Inspectors do that by default.
             */

            formatters.j = function (v) {
            	try {
            		return JSON.stringify(v);
            	} catch (error) {
            		return '[UnexpectedJSONParseError]: ' + error.message;
            	}
            };
            });
            var browser_1 = browser$1.formatArgs;
            var browser_2 = browser$1.save;
            var browser_3 = browser$1.load;
            var browser_4 = browser$1.useColors;
            var browser_5 = browser$1.storage;
            var browser_6 = browser$1.destroy;
            var browser_7 = browser$1.colors;
            var browser_8 = browser$1.log;

            var debugFunc;
            var phase = 'default';
            var namespace = '';

            var newDebug = function newDebug() {
              debugFunc = namespace ? browser$1("fetch-mock:".concat(phase, ":").concat(namespace)) : browser$1("fetch-mock:".concat(phase));
            };

            var newDebugSandbox = function newDebugSandbox(ns) {
              return browser$1("fetch-mock:".concat(phase, ":").concat(ns));
            };

            newDebug();
            var debug_1 = {
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

            var debug = debug_1.debug,
                setDebugPhase = debug_1.setDebugPhase;

            var FetchMock = {};

            FetchMock.mock = function () {
              setDebugPhase('setup');

              for (var _len = arguments.length, args = new Array(_len), _key = 0; _key < _len; _key++) {
                args[_key] = arguments[_key];
              }

              if (args.length) {
                this.addRoute(args);
              }

              return this._mock();
            };

            FetchMock.addRoute = function (uncompiledRoute) {
              var _this = this;

              debug('Adding route', uncompiledRoute);
              var route = this.compileRoute(uncompiledRoute);
              var clashes = this.routes.filter(function (_ref) {
                var identifier = _ref.identifier,
                    method = _ref.method;
                var isMatch = typeof identifier === 'function' ? identifier === route.identifier : String(identifier) === String(route.identifier);
                return isMatch && (!method || !route.method || method === route.method);
              });

              if (this.getOption('overwriteRoutes', route) === false || !clashes.length) {
                this._uncompiledRoutes.push(uncompiledRoute);

                return this.routes.push(route);
              }

              if (this.getOption('overwriteRoutes', route) === true) {
                clashes.forEach(function (clash) {
                  var index = _this.routes.indexOf(clash);

                  _this._uncompiledRoutes.splice(index, 1, uncompiledRoute);

                  _this.routes.splice(index, 1, route);
                });
                return this.routes;
              }

              if (clashes.length) {
                throw new Error('fetch-mock: Adding route with same name or matcher as existing route. See `overwriteRoutes` option.');
              }

              this._uncompiledRoutes.push(uncompiledRoute);

              this.routes.push(route);
            };

            FetchMock._mock = function () {
              if (!this.isSandbox) {
                // Do this here rather than in the constructor to ensure it's scoped to the test
                this.realFetch = this.realFetch || this.global.fetch;
                this.global.fetch = this.fetchHandler;
              }

              setDebugPhase();
              return this;
            };

            FetchMock["catch"] = function (response) {
              if (this.fallbackResponse) {
                console.warn('calling fetchMock.catch() twice - are you sure you want to overwrite the previous fallback response'); // eslint-disable-line
              }

              this.fallbackResponse = response || 'ok';
              return this._mock();
            };

            FetchMock.spy = function (route) {
              // even though ._mock() is called by .mock() and .catch() we still need to
              // call it here otherwise .getNativeFetch() won't be able to use the reference
              // to .realFetch that ._mock() sets up
              this._mock();

              return route ? this.mock(route, this.getNativeFetch()) : this["catch"](this.getNativeFetch());
            };

            var defineShorthand = function defineShorthand(methodName, underlyingMethod, shorthandOptions) {
              FetchMock[methodName] = function (matcher, response, options) {
                return this[underlyingMethod](matcher, response, Object.assign(options || {}, shorthandOptions));
              };
            };

            var defineGreedyShorthand = function defineGreedyShorthand(methodName, underlyingMethod) {
              FetchMock[methodName] = function (response, options) {
                return this[underlyingMethod]({}, response, options);
              };
            };

            defineShorthand('sticky', 'mock', {
              sticky: true
            });
            defineShorthand('once', 'mock', {
              repeat: 1
            });
            defineGreedyShorthand('any', 'mock');
            defineGreedyShorthand('anyOnce', 'once');
            ['get', 'post', 'put', 'delete', 'head', 'patch'].forEach(function (method) {
              defineShorthand(method, 'mock', {
                method: method
              });
              defineShorthand("".concat(method, "Once"), 'once', {
                method: method
              });
              defineGreedyShorthand("".concat(method, "Any"), method);
              defineGreedyShorthand("".concat(method, "AnyOnce"), "".concat(method, "Once"));
            });

            var mochaAsyncHookWorkaround = function mochaAsyncHookWorkaround(options) {
              // HACK workaround for this https://github.com/mochajs/mocha/issues/4280
              // Note that it doesn't matter that we call it _before_ carrying out all
              // the things resetBehavior does as everything in there is synchronous
              if (typeof options === 'function') {
                console.warn("Deprecated: Passing fetch-mock reset methods\ndirectly in as handlers for before/after test runner hooks.\nWrap in an arrow function instead e.g. `() => fetchMock.restore()`");
                options();
              }
            };

            var getRouteRemover = function getRouteRemover(_ref2) {
              var removeStickyRoutes = _ref2.sticky;
              return function (routes) {
                return removeStickyRoutes ? [] : routes.filter(function (_ref3) {
                  var sticky = _ref3.sticky;
                  return sticky;
                });
              };
            };

            FetchMock.resetBehavior = function () {
              var options = arguments.length > 0 && arguments[0] !== undefined ? arguments[0] : {};
              mochaAsyncHookWorkaround(options);
              var removeRoutes = getRouteRemover(options);
              this.routes = removeRoutes(this.routes);
              this._uncompiledRoutes = removeRoutes(this._uncompiledRoutes);

              if (this.realFetch && !this.routes.length) {
                this.global.fetch = this.realFetch;
                this.realFetch = undefined;
              }

              this.fallbackResponse = undefined;
              return this;
            };

            FetchMock.resetHistory = function () {
              this._calls = [];
              this._holdingPromises = [];
              this.routes.forEach(function (route) {
                return route.reset && route.reset();
              });
              return this;
            };

            FetchMock.restore = FetchMock.reset = function (options) {
              this.resetBehavior(options);
              this.resetHistory();
              return this;
            };

            var setUpAndTearDown = FetchMock;

            var interopRequireDefault = createCommonjsModule(function (module) {
            function _interopRequireDefault(obj) {
              return obj && obj.__esModule ? obj : {
                "default": obj
              };
            }

            module.exports = _interopRequireDefault;
            });

            unwrapExports(interopRequireDefault);

            function _arrayWithHoles(arr) {
              if (Array.isArray(arr)) return arr;
            }

            var arrayWithHoles = _arrayWithHoles;

            function _iterableToArrayLimit(arr, i) {
              if (typeof Symbol === "undefined" || !(Symbol.iterator in Object(arr))) return;
              var _arr = [];
              var _n = true;
              var _d = false;
              var _e = undefined;

              try {
                for (var _i = arr[Symbol.iterator](), _s; !(_n = (_s = _i.next()).done); _n = true) {
                  _arr.push(_s.value);

                  if (i && _arr.length === i) break;
                }
              } catch (err) {
                _d = true;
                _e = err;
              } finally {
                try {
                  if (!_n && _i["return"] != null) _i["return"]();
                } finally {
                  if (_d) throw _e;
                }
              }

              return _arr;
            }

            var iterableToArrayLimit = _iterableToArrayLimit;

            function _arrayLikeToArray(arr, len) {
              if (len == null || len > arr.length) len = arr.length;

              for (var i = 0, arr2 = new Array(len); i < len; i++) {
                arr2[i] = arr[i];
              }

              return arr2;
            }

            var arrayLikeToArray = _arrayLikeToArray;

            function _unsupportedIterableToArray(o, minLen) {
              if (!o) return;
              if (typeof o === "string") return arrayLikeToArray(o, minLen);
              var n = Object.prototype.toString.call(o).slice(8, -1);
              if (n === "Object" && o.constructor) n = o.constructor.name;
              if (n === "Map" || n === "Set") return Array.from(o);
              if (n === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(n)) return arrayLikeToArray(o, minLen);
            }

            var unsupportedIterableToArray = _unsupportedIterableToArray;

            function _nonIterableRest() {
              throw new TypeError("Invalid attempt to destructure non-iterable instance.\nIn order to be iterable, non-array objects must have a [Symbol.iterator]() method.");
            }

            var nonIterableRest = _nonIterableRest;

            function _slicedToArray(arr, i) {
              return arrayWithHoles(arr) || iterableToArrayLimit(arr, i) || unsupportedIterableToArray(arr, i) || nonIterableRest();
            }

            var slicedToArray = _slicedToArray;

            var runtime_1 = createCommonjsModule(function (module) {
            /**
             * Copyright (c) 2014-present, Facebook, Inc.
             *
             * This source code is licensed under the MIT license found in the
             * LICENSE file in the root directory of this source tree.
             */

            var runtime = (function (exports) {

              var Op = Object.prototype;
              var hasOwn = Op.hasOwnProperty;
              var undefined$1; // More compressible than void 0.
              var $Symbol = typeof Symbol === "function" ? Symbol : {};
              var iteratorSymbol = $Symbol.iterator || "@@iterator";
              var asyncIteratorSymbol = $Symbol.asyncIterator || "@@asyncIterator";
              var toStringTagSymbol = $Symbol.toStringTag || "@@toStringTag";

              function define(obj, key, value) {
                Object.defineProperty(obj, key, {
                  value: value,
                  enumerable: true,
                  configurable: true,
                  writable: true
                });
                return obj[key];
              }
              try {
                // IE 8 has a broken Object.defineProperty that only works on DOM objects.
                define({}, "");
              } catch (err) {
                define = function(obj, key, value) {
                  return obj[key] = value;
                };
              }

              function wrap(innerFn, outerFn, self, tryLocsList) {
                // If outerFn provided and outerFn.prototype is a Generator, then outerFn.prototype instanceof Generator.
                var protoGenerator = outerFn && outerFn.prototype instanceof Generator ? outerFn : Generator;
                var generator = Object.create(protoGenerator.prototype);
                var context = new Context(tryLocsList || []);

                // The ._invoke method unifies the implementations of the .next,
                // .throw, and .return methods.
                generator._invoke = makeInvokeMethod(innerFn, self, context);

                return generator;
              }
              exports.wrap = wrap;

              // Try/catch helper to minimize deoptimizations. Returns a completion
              // record like context.tryEntries[i].completion. This interface could
              // have been (and was previously) designed to take a closure to be
              // invoked without arguments, but in all the cases we care about we
              // already have an existing method we want to call, so there's no need
              // to create a new function object. We can even get away with assuming
              // the method takes exactly one argument, since that happens to be true
              // in every case, so we don't have to touch the arguments object. The
              // only additional allocation required is the completion record, which
              // has a stable shape and so hopefully should be cheap to allocate.
              function tryCatch(fn, obj, arg) {
                try {
                  return { type: "normal", arg: fn.call(obj, arg) };
                } catch (err) {
                  return { type: "throw", arg: err };
                }
              }

              var GenStateSuspendedStart = "suspendedStart";
              var GenStateSuspendedYield = "suspendedYield";
              var GenStateExecuting = "executing";
              var GenStateCompleted = "completed";

              // Returning this object from the innerFn has the same effect as
              // breaking out of the dispatch switch statement.
              var ContinueSentinel = {};

              // Dummy constructor functions that we use as the .constructor and
              // .constructor.prototype properties for functions that return Generator
              // objects. For full spec compliance, you may wish to configure your
              // minifier not to mangle the names of these two functions.
              function Generator() {}
              function GeneratorFunction() {}
              function GeneratorFunctionPrototype() {}

              // This is a polyfill for %IteratorPrototype% for environments that
              // don't natively support it.
              var IteratorPrototype = {};
              IteratorPrototype[iteratorSymbol] = function () {
                return this;
              };

              var getProto = Object.getPrototypeOf;
              var NativeIteratorPrototype = getProto && getProto(getProto(values([])));
              if (NativeIteratorPrototype &&
                  NativeIteratorPrototype !== Op &&
                  hasOwn.call(NativeIteratorPrototype, iteratorSymbol)) {
                // This environment has a native %IteratorPrototype%; use it instead
                // of the polyfill.
                IteratorPrototype = NativeIteratorPrototype;
              }

              var Gp = GeneratorFunctionPrototype.prototype =
                Generator.prototype = Object.create(IteratorPrototype);
              GeneratorFunction.prototype = Gp.constructor = GeneratorFunctionPrototype;
              GeneratorFunctionPrototype.constructor = GeneratorFunction;
              GeneratorFunction.displayName = define(
                GeneratorFunctionPrototype,
                toStringTagSymbol,
                "GeneratorFunction"
              );

              // Helper for defining the .next, .throw, and .return methods of the
              // Iterator interface in terms of a single ._invoke method.
              function defineIteratorMethods(prototype) {
                ["next", "throw", "return"].forEach(function(method) {
                  define(prototype, method, function(arg) {
                    return this._invoke(method, arg);
                  });
                });
              }

              exports.isGeneratorFunction = function(genFun) {
                var ctor = typeof genFun === "function" && genFun.constructor;
                return ctor
                  ? ctor === GeneratorFunction ||
                    // For the native GeneratorFunction constructor, the best we can
                    // do is to check its .name property.
                    (ctor.displayName || ctor.name) === "GeneratorFunction"
                  : false;
              };

              exports.mark = function(genFun) {
                if (Object.setPrototypeOf) {
                  Object.setPrototypeOf(genFun, GeneratorFunctionPrototype);
                } else {
                  genFun.__proto__ = GeneratorFunctionPrototype;
                  define(genFun, toStringTagSymbol, "GeneratorFunction");
                }
                genFun.prototype = Object.create(Gp);
                return genFun;
              };

              // Within the body of any async function, `await x` is transformed to
              // `yield regeneratorRuntime.awrap(x)`, so that the runtime can test
              // `hasOwn.call(value, "__await")` to determine if the yielded value is
              // meant to be awaited.
              exports.awrap = function(arg) {
                return { __await: arg };
              };

              function AsyncIterator(generator, PromiseImpl) {
                function invoke(method, arg, resolve, reject) {
                  var record = tryCatch(generator[method], generator, arg);
                  if (record.type === "throw") {
                    reject(record.arg);
                  } else {
                    var result = record.arg;
                    var value = result.value;
                    if (value &&
                        typeof value === "object" &&
                        hasOwn.call(value, "__await")) {
                      return PromiseImpl.resolve(value.__await).then(function(value) {
                        invoke("next", value, resolve, reject);
                      }, function(err) {
                        invoke("throw", err, resolve, reject);
                      });
                    }

                    return PromiseImpl.resolve(value).then(function(unwrapped) {
                      // When a yielded Promise is resolved, its final value becomes
                      // the .value of the Promise<{value,done}> result for the
                      // current iteration.
                      result.value = unwrapped;
                      resolve(result);
                    }, function(error) {
                      // If a rejected Promise was yielded, throw the rejection back
                      // into the async generator function so it can be handled there.
                      return invoke("throw", error, resolve, reject);
                    });
                  }
                }

                var previousPromise;

                function enqueue(method, arg) {
                  function callInvokeWithMethodAndArg() {
                    return new PromiseImpl(function(resolve, reject) {
                      invoke(method, arg, resolve, reject);
                    });
                  }

                  return previousPromise =
                    // If enqueue has been called before, then we want to wait until
                    // all previous Promises have been resolved before calling invoke,
                    // so that results are always delivered in the correct order. If
                    // enqueue has not been called before, then it is important to
                    // call invoke immediately, without waiting on a callback to fire,
                    // so that the async generator function has the opportunity to do
                    // any necessary setup in a predictable way. This predictability
                    // is why the Promise constructor synchronously invokes its
                    // executor callback, and why async functions synchronously
                    // execute code before the first await. Since we implement simple
                    // async functions in terms of async generators, it is especially
                    // important to get this right, even though it requires care.
                    previousPromise ? previousPromise.then(
                      callInvokeWithMethodAndArg,
                      // Avoid propagating failures to Promises returned by later
                      // invocations of the iterator.
                      callInvokeWithMethodAndArg
                    ) : callInvokeWithMethodAndArg();
                }

                // Define the unified helper method that is used to implement .next,
                // .throw, and .return (see defineIteratorMethods).
                this._invoke = enqueue;
              }

              defineIteratorMethods(AsyncIterator.prototype);
              AsyncIterator.prototype[asyncIteratorSymbol] = function () {
                return this;
              };
              exports.AsyncIterator = AsyncIterator;

              // Note that simple async functions are implemented on top of
              // AsyncIterator objects; they just return a Promise for the value of
              // the final result produced by the iterator.
              exports.async = function(innerFn, outerFn, self, tryLocsList, PromiseImpl) {
                if (PromiseImpl === void 0) PromiseImpl = Promise;

                var iter = new AsyncIterator(
                  wrap(innerFn, outerFn, self, tryLocsList),
                  PromiseImpl
                );

                return exports.isGeneratorFunction(outerFn)
                  ? iter // If outerFn is a generator, return the full iterator.
                  : iter.next().then(function(result) {
                      return result.done ? result.value : iter.next();
                    });
              };

              function makeInvokeMethod(innerFn, self, context) {
                var state = GenStateSuspendedStart;

                return function invoke(method, arg) {
                  if (state === GenStateExecuting) {
                    throw new Error("Generator is already running");
                  }

                  if (state === GenStateCompleted) {
                    if (method === "throw") {
                      throw arg;
                    }

                    // Be forgiving, per 25.3.3.3.3 of the spec:
                    // https://people.mozilla.org/~jorendorff/es6-draft.html#sec-generatorresume
                    return doneResult();
                  }

                  context.method = method;
                  context.arg = arg;

                  while (true) {
                    var delegate = context.delegate;
                    if (delegate) {
                      var delegateResult = maybeInvokeDelegate(delegate, context);
                      if (delegateResult) {
                        if (delegateResult === ContinueSentinel) continue;
                        return delegateResult;
                      }
                    }

                    if (context.method === "next") {
                      // Setting context._sent for legacy support of Babel's
                      // function.sent implementation.
                      context.sent = context._sent = context.arg;

                    } else if (context.method === "throw") {
                      if (state === GenStateSuspendedStart) {
                        state = GenStateCompleted;
                        throw context.arg;
                      }

                      context.dispatchException(context.arg);

                    } else if (context.method === "return") {
                      context.abrupt("return", context.arg);
                    }

                    state = GenStateExecuting;

                    var record = tryCatch(innerFn, self, context);
                    if (record.type === "normal") {
                      // If an exception is thrown from innerFn, we leave state ===
                      // GenStateExecuting and loop back for another invocation.
                      state = context.done
                        ? GenStateCompleted
                        : GenStateSuspendedYield;

                      if (record.arg === ContinueSentinel) {
                        continue;
                      }

                      return {
                        value: record.arg,
                        done: context.done
                      };

                    } else if (record.type === "throw") {
                      state = GenStateCompleted;
                      // Dispatch the exception by looping back around to the
                      // context.dispatchException(context.arg) call above.
                      context.method = "throw";
                      context.arg = record.arg;
                    }
                  }
                };
              }

              // Call delegate.iterator[context.method](context.arg) and handle the
              // result, either by returning a { value, done } result from the
              // delegate iterator, or by modifying context.method and context.arg,
              // setting context.delegate to null, and returning the ContinueSentinel.
              function maybeInvokeDelegate(delegate, context) {
                var method = delegate.iterator[context.method];
                if (method === undefined$1) {
                  // A .throw or .return when the delegate iterator has no .throw
                  // method always terminates the yield* loop.
                  context.delegate = null;

                  if (context.method === "throw") {
                    // Note: ["return"] must be used for ES3 parsing compatibility.
                    if (delegate.iterator["return"]) {
                      // If the delegate iterator has a return method, give it a
                      // chance to clean up.
                      context.method = "return";
                      context.arg = undefined$1;
                      maybeInvokeDelegate(delegate, context);

                      if (context.method === "throw") {
                        // If maybeInvokeDelegate(context) changed context.method from
                        // "return" to "throw", let that override the TypeError below.
                        return ContinueSentinel;
                      }
                    }

                    context.method = "throw";
                    context.arg = new TypeError(
                      "The iterator does not provide a 'throw' method");
                  }

                  return ContinueSentinel;
                }

                var record = tryCatch(method, delegate.iterator, context.arg);

                if (record.type === "throw") {
                  context.method = "throw";
                  context.arg = record.arg;
                  context.delegate = null;
                  return ContinueSentinel;
                }

                var info = record.arg;

                if (! info) {
                  context.method = "throw";
                  context.arg = new TypeError("iterator result is not an object");
                  context.delegate = null;
                  return ContinueSentinel;
                }

                if (info.done) {
                  // Assign the result of the finished delegate to the temporary
                  // variable specified by delegate.resultName (see delegateYield).
                  context[delegate.resultName] = info.value;

                  // Resume execution at the desired location (see delegateYield).
                  context.next = delegate.nextLoc;

                  // If context.method was "throw" but the delegate handled the
                  // exception, let the outer generator proceed normally. If
                  // context.method was "next", forget context.arg since it has been
                  // "consumed" by the delegate iterator. If context.method was
                  // "return", allow the original .return call to continue in the
                  // outer generator.
                  if (context.method !== "return") {
                    context.method = "next";
                    context.arg = undefined$1;
                  }

                } else {
                  // Re-yield the result returned by the delegate method.
                  return info;
                }

                // The delegate iterator is finished, so forget it and continue with
                // the outer generator.
                context.delegate = null;
                return ContinueSentinel;
              }

              // Define Generator.prototype.{next,throw,return} in terms of the
              // unified ._invoke helper method.
              defineIteratorMethods(Gp);

              define(Gp, toStringTagSymbol, "Generator");

              // A Generator should always return itself as the iterator object when the
              // @@iterator function is called on it. Some browsers' implementations of the
              // iterator prototype chain incorrectly implement this, causing the Generator
              // object to not be returned from this call. This ensures that doesn't happen.
              // See https://github.com/facebook/regenerator/issues/274 for more details.
              Gp[iteratorSymbol] = function() {
                return this;
              };

              Gp.toString = function() {
                return "[object Generator]";
              };

              function pushTryEntry(locs) {
                var entry = { tryLoc: locs[0] };

                if (1 in locs) {
                  entry.catchLoc = locs[1];
                }

                if (2 in locs) {
                  entry.finallyLoc = locs[2];
                  entry.afterLoc = locs[3];
                }

                this.tryEntries.push(entry);
              }

              function resetTryEntry(entry) {
                var record = entry.completion || {};
                record.type = "normal";
                delete record.arg;
                entry.completion = record;
              }

              function Context(tryLocsList) {
                // The root entry object (effectively a try statement without a catch
                // or a finally block) gives us a place to store values thrown from
                // locations where there is no enclosing try statement.
                this.tryEntries = [{ tryLoc: "root" }];
                tryLocsList.forEach(pushTryEntry, this);
                this.reset(true);
              }

              exports.keys = function(object) {
                var keys = [];
                for (var key in object) {
                  keys.push(key);
                }
                keys.reverse();

                // Rather than returning an object with a next method, we keep
                // things simple and return the next function itself.
                return function next() {
                  while (keys.length) {
                    var key = keys.pop();
                    if (key in object) {
                      next.value = key;
                      next.done = false;
                      return next;
                    }
                  }

                  // To avoid creating an additional object, we just hang the .value
                  // and .done properties off the next function object itself. This
                  // also ensures that the minifier will not anonymize the function.
                  next.done = true;
                  return next;
                };
              };

              function values(iterable) {
                if (iterable) {
                  var iteratorMethod = iterable[iteratorSymbol];
                  if (iteratorMethod) {
                    return iteratorMethod.call(iterable);
                  }

                  if (typeof iterable.next === "function") {
                    return iterable;
                  }

                  if (!isNaN(iterable.length)) {
                    var i = -1, next = function next() {
                      while (++i < iterable.length) {
                        if (hasOwn.call(iterable, i)) {
                          next.value = iterable[i];
                          next.done = false;
                          return next;
                        }
                      }

                      next.value = undefined$1;
                      next.done = true;

                      return next;
                    };

                    return next.next = next;
                  }
                }

                // Return an iterator with no values.
                return { next: doneResult };
              }
              exports.values = values;

              function doneResult() {
                return { value: undefined$1, done: true };
              }

              Context.prototype = {
                constructor: Context,

                reset: function(skipTempReset) {
                  this.prev = 0;
                  this.next = 0;
                  // Resetting context._sent for legacy support of Babel's
                  // function.sent implementation.
                  this.sent = this._sent = undefined$1;
                  this.done = false;
                  this.delegate = null;

                  this.method = "next";
                  this.arg = undefined$1;

                  this.tryEntries.forEach(resetTryEntry);

                  if (!skipTempReset) {
                    for (var name in this) {
                      // Not sure about the optimal order of these conditions:
                      if (name.charAt(0) === "t" &&
                          hasOwn.call(this, name) &&
                          !isNaN(+name.slice(1))) {
                        this[name] = undefined$1;
                      }
                    }
                  }
                },

                stop: function() {
                  this.done = true;

                  var rootEntry = this.tryEntries[0];
                  var rootRecord = rootEntry.completion;
                  if (rootRecord.type === "throw") {
                    throw rootRecord.arg;
                  }

                  return this.rval;
                },

                dispatchException: function(exception) {
                  if (this.done) {
                    throw exception;
                  }

                  var context = this;
                  function handle(loc, caught) {
                    record.type = "throw";
                    record.arg = exception;
                    context.next = loc;

                    if (caught) {
                      // If the dispatched exception was caught by a catch block,
                      // then let that catch block handle the exception normally.
                      context.method = "next";
                      context.arg = undefined$1;
                    }

                    return !! caught;
                  }

                  for (var i = this.tryEntries.length - 1; i >= 0; --i) {
                    var entry = this.tryEntries[i];
                    var record = entry.completion;

                    if (entry.tryLoc === "root") {
                      // Exception thrown outside of any try block that could handle
                      // it, so set the completion value of the entire function to
                      // throw the exception.
                      return handle("end");
                    }

                    if (entry.tryLoc <= this.prev) {
                      var hasCatch = hasOwn.call(entry, "catchLoc");
                      var hasFinally = hasOwn.call(entry, "finallyLoc");

                      if (hasCatch && hasFinally) {
                        if (this.prev < entry.catchLoc) {
                          return handle(entry.catchLoc, true);
                        } else if (this.prev < entry.finallyLoc) {
                          return handle(entry.finallyLoc);
                        }

                      } else if (hasCatch) {
                        if (this.prev < entry.catchLoc) {
                          return handle(entry.catchLoc, true);
                        }

                      } else if (hasFinally) {
                        if (this.prev < entry.finallyLoc) {
                          return handle(entry.finallyLoc);
                        }

                      } else {
                        throw new Error("try statement without catch or finally");
                      }
                    }
                  }
                },

                abrupt: function(type, arg) {
                  for (var i = this.tryEntries.length - 1; i >= 0; --i) {
                    var entry = this.tryEntries[i];
                    if (entry.tryLoc <= this.prev &&
                        hasOwn.call(entry, "finallyLoc") &&
                        this.prev < entry.finallyLoc) {
                      var finallyEntry = entry;
                      break;
                    }
                  }

                  if (finallyEntry &&
                      (type === "break" ||
                       type === "continue") &&
                      finallyEntry.tryLoc <= arg &&
                      arg <= finallyEntry.finallyLoc) {
                    // Ignore the finally entry if control is not jumping to a
                    // location outside the try/catch block.
                    finallyEntry = null;
                  }

                  var record = finallyEntry ? finallyEntry.completion : {};
                  record.type = type;
                  record.arg = arg;

                  if (finallyEntry) {
                    this.method = "next";
                    this.next = finallyEntry.finallyLoc;
                    return ContinueSentinel;
                  }

                  return this.complete(record);
                },

                complete: function(record, afterLoc) {
                  if (record.type === "throw") {
                    throw record.arg;
                  }

                  if (record.type === "break" ||
                      record.type === "continue") {
                    this.next = record.arg;
                  } else if (record.type === "return") {
                    this.rval = this.arg = record.arg;
                    this.method = "return";
                    this.next = "end";
                  } else if (record.type === "normal" && afterLoc) {
                    this.next = afterLoc;
                  }

                  return ContinueSentinel;
                },

                finish: function(finallyLoc) {
                  for (var i = this.tryEntries.length - 1; i >= 0; --i) {
                    var entry = this.tryEntries[i];
                    if (entry.finallyLoc === finallyLoc) {
                      this.complete(entry.completion, entry.afterLoc);
                      resetTryEntry(entry);
                      return ContinueSentinel;
                    }
                  }
                },

                "catch": function(tryLoc) {
                  for (var i = this.tryEntries.length - 1; i >= 0; --i) {
                    var entry = this.tryEntries[i];
                    if (entry.tryLoc === tryLoc) {
                      var record = entry.completion;
                      if (record.type === "throw") {
                        var thrown = record.arg;
                        resetTryEntry(entry);
                      }
                      return thrown;
                    }
                  }

                  // The context.catch method must only be called with a location
                  // argument that corresponds to a known catch block.
                  throw new Error("illegal catch attempt");
                },

                delegateYield: function(iterable, resultName, nextLoc) {
                  this.delegate = {
                    iterator: values(iterable),
                    resultName: resultName,
                    nextLoc: nextLoc
                  };

                  if (this.method === "next") {
                    // Deliberately forget the last sent value so that we don't
                    // accidentally pass it on to the delegate.
                    this.arg = undefined$1;
                  }

                  return ContinueSentinel;
                }
              };

              // Regardless of whether this script is executing as a CommonJS module
              // or not, return the runtime object so that we can declare the variable
              // regeneratorRuntime in the outer scope, which allows this module to be
              // injected easily by `bin/regenerator --include-runtime script.js`.
              return exports;

            }(
              // If this script is executing as a CommonJS module, use module.exports
              // as the regeneratorRuntime namespace. Otherwise create a new empty
              // object. Either way, the resulting object will be used to initialize
              // the regeneratorRuntime variable at the top of this file.
               module.exports 
            ));

            try {
              regeneratorRuntime = runtime;
            } catch (accidentalStrictMode) {
              // This module should not be running in strict mode, so the above
              // assignment should always work unless something is misconfigured. Just
              // in case runtime.js accidentally runs in strict mode, we can escape
              // strict mode using a global Function call. This could conceivably fail
              // if a Content Security Policy forbids using Function, but in that case
              // the proper solution is to fix the accidental strict mode problem. If
              // you've misconfigured your bundler to force strict mode and applied a
              // CSP to forbid Function, and you're not willing to fix either of those
              // problems, please detail your unique predicament in a GitHub issue.
              Function("r", "regeneratorRuntime = r")(runtime);
            }
            });

            var regenerator = runtime_1;

            function asyncGeneratorStep(gen, resolve, reject, _next, _throw, key, arg) {
              try {
                var info = gen[key](arg);
                var value = info.value;
              } catch (error) {
                reject(error);
                return;
              }

              if (info.done) {
                resolve(value);
              } else {
                Promise.resolve(value).then(_next, _throw);
              }
            }

            function _asyncToGenerator(fn) {
              return function () {
                var self = this,
                    args = arguments;
                return new Promise(function (resolve, reject) {
                  var gen = fn.apply(self, args);

                  function _next(value) {
                    asyncGeneratorStep(gen, resolve, reject, _next, _throw, "next", value);
                  }

                  function _throw(err) {
                    asyncGeneratorStep(gen, resolve, reject, _next, _throw, "throw", err);
                  }

                  _next(undefined);
                });
              };
            }

            var asyncToGenerator = _asyncToGenerator;

            function _classCallCheck(instance, Constructor) {
              if (!(instance instanceof Constructor)) {
                throw new TypeError("Cannot call a class as a function");
              }
            }

            var classCallCheck = _classCallCheck;

            function _assertThisInitialized(self) {
              if (self === void 0) {
                throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
              }

              return self;
            }

            var assertThisInitialized = _assertThisInitialized;

            var setPrototypeOf = createCommonjsModule(function (module) {
            function _setPrototypeOf(o, p) {
              module.exports = _setPrototypeOf = Object.setPrototypeOf || function _setPrototypeOf(o, p) {
                o.__proto__ = p;
                return o;
              };

              return _setPrototypeOf(o, p);
            }

            module.exports = _setPrototypeOf;
            });

            function _inherits(subClass, superClass) {
              if (typeof superClass !== "function" && superClass !== null) {
                throw new TypeError("Super expression must either be null or a function");
              }

              subClass.prototype = Object.create(superClass && superClass.prototype, {
                constructor: {
                  value: subClass,
                  writable: true,
                  configurable: true
                }
              });
              if (superClass) setPrototypeOf(subClass, superClass);
            }

            var inherits = _inherits;

            var _typeof_1 = createCommonjsModule(function (module) {
            function _typeof(obj) {
              "@babel/helpers - typeof";

              if (typeof Symbol === "function" && typeof Symbol.iterator === "symbol") {
                module.exports = _typeof = function _typeof(obj) {
                  return typeof obj;
                };
              } else {
                module.exports = _typeof = function _typeof(obj) {
                  return obj && typeof Symbol === "function" && obj.constructor === Symbol && obj !== Symbol.prototype ? "symbol" : typeof obj;
                };
              }

              return _typeof(obj);
            }

            module.exports = _typeof;
            });

            function _possibleConstructorReturn(self, call) {
              if (call && (_typeof_1(call) === "object" || typeof call === "function")) {
                return call;
              }

              return assertThisInitialized(self);
            }

            var possibleConstructorReturn = _possibleConstructorReturn;

            var getPrototypeOf = createCommonjsModule(function (module) {
            function _getPrototypeOf(o) {
              module.exports = _getPrototypeOf = Object.setPrototypeOf ? Object.getPrototypeOf : function _getPrototypeOf(o) {
                return o.__proto__ || Object.getPrototypeOf(o);
              };
              return _getPrototypeOf(o);
            }

            module.exports = _getPrototypeOf;
            });

            function _isNativeFunction(fn) {
              return Function.toString.call(fn).indexOf("[native code]") !== -1;
            }

            var isNativeFunction = _isNativeFunction;

            function _isNativeReflectConstruct() {
              if (typeof Reflect === "undefined" || !Reflect.construct) return false;
              if (Reflect.construct.sham) return false;
              if (typeof Proxy === "function") return true;

              try {
                Date.prototype.toString.call(Reflect.construct(Date, [], function () {}));
                return true;
              } catch (e) {
                return false;
              }
            }

            var isNativeReflectConstruct = _isNativeReflectConstruct;

            var construct = createCommonjsModule(function (module) {
            function _construct(Parent, args, Class) {
              if (isNativeReflectConstruct()) {
                module.exports = _construct = Reflect.construct;
              } else {
                module.exports = _construct = function _construct(Parent, args, Class) {
                  var a = [null];
                  a.push.apply(a, args);
                  var Constructor = Function.bind.apply(Parent, a);
                  var instance = new Constructor();
                  if (Class) setPrototypeOf(instance, Class.prototype);
                  return instance;
                };
              }

              return _construct.apply(null, arguments);
            }

            module.exports = _construct;
            });

            var wrapNativeSuper = createCommonjsModule(function (module) {
            function _wrapNativeSuper(Class) {
              var _cache = typeof Map === "function" ? new Map() : undefined;

              module.exports = _wrapNativeSuper = function _wrapNativeSuper(Class) {
                if (Class === null || !isNativeFunction(Class)) return Class;

                if (typeof Class !== "function") {
                  throw new TypeError("Super expression must either be null or a function");
                }

                if (typeof _cache !== "undefined") {
                  if (_cache.has(Class)) return _cache.get(Class);

                  _cache.set(Class, Wrapper);
                }

                function Wrapper() {
                  return construct(Class, arguments, getPrototypeOf(this).constructor);
                }

                Wrapper.prototype = Object.create(Class.prototype, {
                  constructor: {
                    value: Wrapper,
                    enumerable: false,
                    writable: true,
                    configurable: true
                  }
                });
                return setPrototypeOf(Wrapper, Class);
              };

              return _wrapNativeSuper(Class);
            }

            module.exports = _wrapNativeSuper;
            });

            function _defineProperties(target, props) {
              for (var i = 0; i < props.length; i++) {
                var descriptor = props[i];
                descriptor.enumerable = descriptor.enumerable || false;
                descriptor.configurable = true;
                if ("value" in descriptor) descriptor.writable = true;
                Object.defineProperty(target, descriptor.key, descriptor);
              }
            }

            function _createClass(Constructor, protoProps, staticProps) {
              if (protoProps) _defineProperties(Constructor.prototype, protoProps);
              if (staticProps) _defineProperties(Constructor, staticProps);
              return Constructor;
            }

            var createClass = _createClass;

            var _typeof2 = interopRequireDefault(_typeof_1);

            var _classCallCheck2 = interopRequireDefault(classCallCheck);

            var _createClass2 = interopRequireDefault(createClass);

            var getDebug = debug_1.getDebug;

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

            var responseBuilder = function (options) {
              return new ResponseBuilder(options).exec();
            };

            function _defineProperty(obj, key, value) {
              if (key in obj) {
                Object.defineProperty(obj, key, {
                  value: value,
                  enumerable: true,
                  configurable: true,
                  writable: true
                });
              } else {
                obj[key] = value;
              }

              return obj;
            }

            var defineProperty = _defineProperty;

            function _arrayWithoutHoles(arr) {
              if (Array.isArray(arr)) return arrayLikeToArray(arr);
            }

            var arrayWithoutHoles = _arrayWithoutHoles;

            function _iterableToArray(iter) {
              if (typeof Symbol !== "undefined" && Symbol.iterator in Object(iter)) return Array.from(iter);
            }

            var iterableToArray = _iterableToArray;

            function _nonIterableSpread() {
              throw new TypeError("Invalid attempt to spread non-iterable instance.\nIn order to be iterable, non-array objects must have a [Symbol.iterator]() method.");
            }

            var nonIterableSpread = _nonIterableSpread;

            function _toConsumableArray(arr) {
              return arrayWithoutHoles(arr) || iterableToArray(arr) || unsupportedIterableToArray(arr) || nonIterableSpread();
            }

            var toConsumableArray = _toConsumableArray;

            var _typeof2$1 = interopRequireDefault(_typeof_1);

            var _regenerator = interopRequireDefault(regenerator);

            var _asyncToGenerator2 = interopRequireDefault(asyncToGenerator);

            var _defineProperty2 = interopRequireDefault(defineProperty);

            var _slicedToArray2 = interopRequireDefault(slicedToArray);

            var _toConsumableArray2 = interopRequireDefault(toConsumableArray);

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

            var requestUtils = {
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
                (0, _typeof2$1["default"])(url) === 'object' && 'href' in url) {
                  return {
                    url: normalizeUrl(url),
                    options: options,
                    signal: options && options.signal
                  };
                } else if ((0, _typeof2$1["default"])(url) === 'object') {
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

            var _slicedToArray2$1 = interopRequireDefault(slicedToArray);

            var _regenerator$1 = interopRequireDefault(regenerator);

            var _asyncToGenerator2$1 = interopRequireDefault(asyncToGenerator);

            var _classCallCheck2$1 = interopRequireDefault(classCallCheck);

            var _assertThisInitialized2 = interopRequireDefault(assertThisInitialized);

            var _inherits2 = interopRequireDefault(inherits);

            var _possibleConstructorReturn2 = interopRequireDefault(possibleConstructorReturn);

            var _getPrototypeOf2 = interopRequireDefault(getPrototypeOf);

            var _wrapNativeSuper2 = interopRequireDefault(wrapNativeSuper);

            function _createSuper(Derived) { var hasNativeReflectConstruct = _isNativeReflectConstruct$1(); return function _createSuperInternal() { var Super = (0, _getPrototypeOf2["default"])(Derived), result; if (hasNativeReflectConstruct) { var NewTarget = (0, _getPrototypeOf2["default"])(this).constructor; result = Reflect.construct(Super, arguments, NewTarget); } else { result = Super.apply(this, arguments); } return (0, _possibleConstructorReturn2["default"])(this, result); }; }

            function _isNativeReflectConstruct$1() { if (typeof Reflect === "undefined" || !Reflect.construct) return false; if (Reflect.construct.sham) return false; if (typeof Proxy === "function") return true; try { Date.prototype.toString.call(Reflect.construct(Date, [], function () {})); return true; } catch (e) { return false; } }

            var debug$1 = debug_1.debug,
                setDebugPhase$1 = debug_1.setDebugPhase,
                getDebug$1 = debug_1.getDebug;





            var FetchMock$1 = {}; // see https://heycam.github.io/webidl/#aborterror for the standardised interface
            // Note that this differs slightly from node-fetch

            var AbortError = /*#__PURE__*/function (_Error) {
              (0, _inherits2["default"])(AbortError, _Error);

              var _super = _createSuper(AbortError);

              function AbortError() {
                var _this;

                (0, _classCallCheck2$1["default"])(this, AbortError);
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
                var _ref = (0, _asyncToGenerator2$1["default"])( /*#__PURE__*/_regenerator$1["default"].mark(function _callee(request) {
                  var method, body, cache, credentials, headers, integrity, mode, redirect, referrer, init;
                  return _regenerator$1["default"].wrap(function _callee$(_context) {
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
              var _ref2 = (0, _asyncToGenerator2$1["default"])( /*#__PURE__*/_regenerator$1["default"].mark(function _callee2(_ref3, url, options, request) {
                var response, _ref3$responseIsFetch, responseIsFetch, debug;

                return _regenerator$1["default"].wrap(function _callee2$(_context2) {
                  while (1) {
                    switch (_context2.prev = _context2.next) {
                      case 0:
                        response = _ref3.response, _ref3$responseIsFetch = _ref3.responseIsFetch, responseIsFetch = _ref3$responseIsFetch === void 0 ? false : _ref3$responseIsFetch;
                        debug = getDebug$1('resolve()');
                        debug('Recursively resolving function and promise responses'); // We want to allow things like
                        // - function returning a Promise for a response
                        // - delaying (using a timeout Promise) a function's execution to generate
                        //   a response
                        // Because of this we can't safely check for function before Promisey-ness,
                        // or vice versa. So to keep it DRY, and flexible, we keep trying until we
                        // have something that looks like neither Promise nor function

                      case 3:

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

            FetchMock$1.needsAsyncBodyExtraction = function (_ref4) {
              var request = _ref4.request;
              return request && this.routes.some(function (_ref5) {
                var usesBody = _ref5.usesBody;
                return usesBody;
              });
            };

            FetchMock$1.fetchHandler = function (url, options) {
              setDebugPhase$1('handle');
              var debug = getDebug$1('fetchHandler()');
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

            FetchMock$1._extractBodyThenHandle = /*#__PURE__*/function () {
              var _ref6 = (0, _asyncToGenerator2$1["default"])( /*#__PURE__*/_regenerator$1["default"].mark(function _callee3(normalizedRequest) {
                return _regenerator$1["default"].wrap(function _callee3$(_context3) {
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

            FetchMock$1._fetchHandler = function (_ref7) {
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
                  debug$1('signal exists - enabling fetch abort');

                  var abort = function abort() {
                    debug$1('aborting fetch'); // note that DOMException is not available in node.js;
                    // even node-fetch uses a custom error class:
                    // https://github.com/bitinn/node-fetch/blob/master/src/abort-error.js

                    rej(typeof DOMException !== 'undefined' ? new DOMException('The operation was aborted.', 'AbortError') : new AbortError());
                    done();
                  };

                  if (signal.aborted) {
                    debug$1('signal is already aborted - aborting the fetch');
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
                  setDebugPhase$1();
                });
              });
            };

            FetchMock$1.fetchHandler.isMock = true;

            FetchMock$1.executeRouter = function (url, options, request) {
              var debug = getDebug$1('executeRouter()');
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

            FetchMock$1.generateResponse = /*#__PURE__*/function () {
              var _ref8 = (0, _asyncToGenerator2$1["default"])( /*#__PURE__*/_regenerator$1["default"].mark(function _callee4(_ref9) {
                var route, url, options, request, _ref9$callLog, callLog, debug, response, _responseBuilder, _responseBuilder2, realResponse, finalResponse;

                return _regenerator$1["default"].wrap(function _callee4$(_context4) {
                  while (1) {
                    switch (_context4.prev = _context4.next) {
                      case 0:
                        route = _ref9.route, url = _ref9.url, options = _ref9.options, request = _ref9.request, _ref9$callLog = _ref9.callLog, callLog = _ref9$callLog === void 0 ? {} : _ref9$callLog;
                        debug = getDebug$1('generateResponse()');
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
                        }), _responseBuilder2 = (0, _slicedToArray2$1["default"])(_responseBuilder, 2), realResponse = _responseBuilder2[0], finalResponse = _responseBuilder2[1];
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

            FetchMock$1.router = function (url, options, request) {
              var route = this.routes.find(function (route, i) {
                debug$1("Trying to match route ".concat(i));
                return route.matcher(url, options, request);
              });

              if (route) {
                return route;
              }
            };

            FetchMock$1.getNativeFetch = function () {
              var func = this.realFetch || this.isSandbox && this.config.fetch;

              if (!func) {
                throw new Error('fetch-mock: Falling back to network only available on global fetch-mock, or by setting config.fetch on sandboxed fetch-mock');
              }

              return patchNativeFetchForSafari(func);
            };

            FetchMock$1.recordCall = function (obj) {
              debug$1('Recording fetch call', obj);

              if (obj) {
                this._calls.push(obj);
              }
            };

            var fetchHandler = FetchMock$1;

            var globToRegexp = function (glob, opts) {
              if (typeof glob !== 'string') {
                throw new TypeError('Expected a string');
              }

              var str = String(glob);

              // The regexp we are building, as a string.
              var reStr = "";

              // Whether we are matching so called "extended" globs (like bash) and should
              // support single character matching, matching ranges of characters, group
              // matching, etc.
              var extended = opts ? !!opts.extended : false;

              // When globstar is _false_ (default), '/foo/*' is translated a regexp like
              // '^\/foo\/.*$' which will match any string beginning with '/foo/'
              // When globstar is _true_, '/foo/*' is translated to regexp like
              // '^\/foo\/[^/]*$' which will match any string beginning with '/foo/' BUT
              // which does not have a '/' to the right of it.
              // E.g. with '/foo/*' these will match: '/foo/bar', '/foo/bar.txt' but
              // these will not '/foo/bar/baz', '/foo/bar/baz.txt'
              // Lastely, when globstar is _true_, '/foo/**' is equivelant to '/foo/*' when
              // globstar is _false_
              var globstar = opts ? !!opts.globstar : false;

              // If we are doing extended matching, this boolean is true when we are inside
              // a group (eg {*.html,*.js}), and false otherwise.
              var inGroup = false;

              // RegExp flags (eg "i" ) to pass in to RegExp constructor.
              var flags = opts && typeof( opts.flags ) === "string" ? opts.flags : "";

              var c;
              for (var i = 0, len = str.length; i < len; i++) {
                c = str[i];

                switch (c) {
                case "/":
                case "$":
                case "^":
                case "+":
                case ".":
                case "(":
                case ")":
                case "=":
                case "!":
                case "|":
                  reStr += "\\" + c;
                  break;

                case "?":
                  if (extended) {
                    reStr += ".";
            	    break;
                  }

                case "[":
                case "]":
                  if (extended) {
                    reStr += c;
            	    break;
                  }

                case "{":
                  if (extended) {
                    inGroup = true;
            	    reStr += "(";
            	    break;
                  }

                case "}":
                  if (extended) {
                    inGroup = false;
            	    reStr += ")";
            	    break;
                  }

                case ",":
                  if (inGroup) {
                    reStr += "|";
            	    break;
                  }
                  reStr += "\\" + c;
                  break;

                case "*":
                  // Move over all consecutive "*"'s.
                  // Also store the previous and next characters
                  var prevChar = str[i - 1];
                  var starCount = 1;
                  while(str[i + 1] === "*") {
                    starCount++;
                    i++;
                  }
                  var nextChar = str[i + 1];

                  if (!globstar) {
                    // globstar is disabled, so treat any number of "*" as one
                    reStr += ".*";
                  } else {
                    // globstar is enabled, so determine if this is a globstar segment
                    var isGlobstar = starCount > 1                      // multiple "*"'s
                      && (prevChar === "/" || prevChar === undefined)   // from the start of the segment
                      && (nextChar === "/" || nextChar === undefined);   // to the end of the segment

                    if (isGlobstar) {
                      // it's a globstar, so match zero or more path segments
                      reStr += "((?:[^/]*(?:\/|$))*)";
                      i++; // move over the "/"
                    } else {
                      // it's not a globstar, so only match one path segment
                      reStr += "([^/]*)";
                    }
                  }
                  break;

                default:
                  reStr += c;
                }
              }

              // When regexp 'g' flag is specified don't
              // constrain the regular expression with ^ & $
              if (!flags || !~flags.indexOf('g')) {
                reStr = "^" + reStr + "$";
              }

              return new RegExp(reStr, flags);
            };

            /**
             * Expose `pathToRegexp`.
             */
            var pathToRegexp_1 = pathToRegexp;
            var parse_1 = parse$1;
            var compile_1 = compile;
            var tokensToFunction_1 = tokensToFunction;
            var tokensToRegExp_1 = tokensToRegExp;

            /**
             * Default configs.
             */
            var DEFAULT_DELIMITER = '/';
            var DEFAULT_DELIMITERS = './';

            /**
             * The main path matching regexp utility.
             *
             * @type {RegExp}
             */
            var PATH_REGEXP = new RegExp([
              // Match escaped characters that would otherwise appear in future matches.
              // This allows the user to escape special characters that won't transform.
              '(\\\\.)',
              // Match Express-style parameters and un-named parameters with a prefix
              // and optional suffixes. Matches appear as:
              //
              // ":test(\\d+)?" => ["test", "\d+", undefined, "?"]
              // "(\\d+)"  => [undefined, undefined, "\d+", undefined]
              '(?:\\:(\\w+)(?:\\(((?:\\\\.|[^\\\\()])+)\\))?|\\(((?:\\\\.|[^\\\\()])+)\\))([+*?])?'
            ].join('|'), 'g');

            /**
             * Parse a string for the raw tokens.
             *
             * @param  {string}  str
             * @param  {Object=} options
             * @return {!Array}
             */
            function parse$1 (str, options) {
              var tokens = [];
              var key = 0;
              var index = 0;
              var path = '';
              var defaultDelimiter = (options && options.delimiter) || DEFAULT_DELIMITER;
              var delimiters = (options && options.delimiters) || DEFAULT_DELIMITERS;
              var pathEscaped = false;
              var res;

              while ((res = PATH_REGEXP.exec(str)) !== null) {
                var m = res[0];
                var escaped = res[1];
                var offset = res.index;
                path += str.slice(index, offset);
                index = offset + m.length;

                // Ignore already escaped sequences.
                if (escaped) {
                  path += escaped[1];
                  pathEscaped = true;
                  continue
                }

                var prev = '';
                var next = str[index];
                var name = res[2];
                var capture = res[3];
                var group = res[4];
                var modifier = res[5];

                if (!pathEscaped && path.length) {
                  var k = path.length - 1;

                  if (delimiters.indexOf(path[k]) > -1) {
                    prev = path[k];
                    path = path.slice(0, k);
                  }
                }

                // Push the current path onto the tokens.
                if (path) {
                  tokens.push(path);
                  path = '';
                  pathEscaped = false;
                }

                var partial = prev !== '' && next !== undefined && next !== prev;
                var repeat = modifier === '+' || modifier === '*';
                var optional = modifier === '?' || modifier === '*';
                var delimiter = prev || defaultDelimiter;
                var pattern = capture || group;

                tokens.push({
                  name: name || key++,
                  prefix: prev,
                  delimiter: delimiter,
                  optional: optional,
                  repeat: repeat,
                  partial: partial,
                  pattern: pattern ? escapeGroup(pattern) : '[^' + escapeString(delimiter) + ']+?'
                });
              }

              // Push any remaining characters.
              if (path || index < str.length) {
                tokens.push(path + str.substr(index));
              }

              return tokens
            }

            /**
             * Compile a string to a template function for the path.
             *
             * @param  {string}             str
             * @param  {Object=}            options
             * @return {!function(Object=, Object=)}
             */
            function compile (str, options) {
              return tokensToFunction(parse$1(str, options))
            }

            /**
             * Expose a method for transforming tokens into the path function.
             */
            function tokensToFunction (tokens) {
              // Compile all the tokens into regexps.
              var matches = new Array(tokens.length);

              // Compile all the patterns before compilation.
              for (var i = 0; i < tokens.length; i++) {
                if (typeof tokens[i] === 'object') {
                  matches[i] = new RegExp('^(?:' + tokens[i].pattern + ')$');
                }
              }

              return function (data, options) {
                var path = '';
                var encode = (options && options.encode) || encodeURIComponent;

                for (var i = 0; i < tokens.length; i++) {
                  var token = tokens[i];

                  if (typeof token === 'string') {
                    path += token;
                    continue
                  }

                  var value = data ? data[token.name] : undefined;
                  var segment;

                  if (Array.isArray(value)) {
                    if (!token.repeat) {
                      throw new TypeError('Expected "' + token.name + '" to not repeat, but got array')
                    }

                    if (value.length === 0) {
                      if (token.optional) continue

                      throw new TypeError('Expected "' + token.name + '" to not be empty')
                    }

                    for (var j = 0; j < value.length; j++) {
                      segment = encode(value[j], token);

                      if (!matches[i].test(segment)) {
                        throw new TypeError('Expected all "' + token.name + '" to match "' + token.pattern + '"')
                      }

                      path += (j === 0 ? token.prefix : token.delimiter) + segment;
                    }

                    continue
                  }

                  if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
                    segment = encode(String(value), token);

                    if (!matches[i].test(segment)) {
                      throw new TypeError('Expected "' + token.name + '" to match "' + token.pattern + '", but got "' + segment + '"')
                    }

                    path += token.prefix + segment;
                    continue
                  }

                  if (token.optional) {
                    // Prepend partial segment prefixes.
                    if (token.partial) path += token.prefix;

                    continue
                  }

                  throw new TypeError('Expected "' + token.name + '" to be ' + (token.repeat ? 'an array' : 'a string'))
                }

                return path
              }
            }

            /**
             * Escape a regular expression string.
             *
             * @param  {string} str
             * @return {string}
             */
            function escapeString (str) {
              return str.replace(/([.+*?=^!:${}()[\]|/\\])/g, '\\$1')
            }

            /**
             * Escape the capturing group by escaping special characters and meaning.
             *
             * @param  {string} group
             * @return {string}
             */
            function escapeGroup (group) {
              return group.replace(/([=!:$/()])/g, '\\$1')
            }

            /**
             * Get the flags for a regexp from the options.
             *
             * @param  {Object} options
             * @return {string}
             */
            function flags (options) {
              return options && options.sensitive ? '' : 'i'
            }

            /**
             * Pull out keys from a regexp.
             *
             * @param  {!RegExp} path
             * @param  {Array=}  keys
             * @return {!RegExp}
             */
            function regexpToRegexp (path, keys) {
              if (!keys) return path

              // Use a negative lookahead to match only capturing groups.
              var groups = path.source.match(/\((?!\?)/g);

              if (groups) {
                for (var i = 0; i < groups.length; i++) {
                  keys.push({
                    name: i,
                    prefix: null,
                    delimiter: null,
                    optional: false,
                    repeat: false,
                    partial: false,
                    pattern: null
                  });
                }
              }

              return path
            }

            /**
             * Transform an array into a regexp.
             *
             * @param  {!Array}  path
             * @param  {Array=}  keys
             * @param  {Object=} options
             * @return {!RegExp}
             */
            function arrayToRegexp (path, keys, options) {
              var parts = [];

              for (var i = 0; i < path.length; i++) {
                parts.push(pathToRegexp(path[i], keys, options).source);
              }

              return new RegExp('(?:' + parts.join('|') + ')', flags(options))
            }

            /**
             * Create a path regexp from string input.
             *
             * @param  {string}  path
             * @param  {Array=}  keys
             * @param  {Object=} options
             * @return {!RegExp}
             */
            function stringToRegexp (path, keys, options) {
              return tokensToRegExp(parse$1(path, options), keys, options)
            }

            /**
             * Expose a function for taking tokens and returning a RegExp.
             *
             * @param  {!Array}  tokens
             * @param  {Array=}  keys
             * @param  {Object=} options
             * @return {!RegExp}
             */
            function tokensToRegExp (tokens, keys, options) {
              options = options || {};

              var strict = options.strict;
              var start = options.start !== false;
              var end = options.end !== false;
              var delimiter = escapeString(options.delimiter || DEFAULT_DELIMITER);
              var delimiters = options.delimiters || DEFAULT_DELIMITERS;
              var endsWith = [].concat(options.endsWith || []).map(escapeString).concat('$').join('|');
              var route = start ? '^' : '';
              var isEndDelimited = tokens.length === 0;

              // Iterate over the tokens and create our regexp string.
              for (var i = 0; i < tokens.length; i++) {
                var token = tokens[i];

                if (typeof token === 'string') {
                  route += escapeString(token);
                  isEndDelimited = i === tokens.length - 1 && delimiters.indexOf(token[token.length - 1]) > -1;
                } else {
                  var capture = token.repeat
                    ? '(?:' + token.pattern + ')(?:' + escapeString(token.delimiter) + '(?:' + token.pattern + '))*'
                    : token.pattern;

                  if (keys) keys.push(token);

                  if (token.optional) {
                    if (token.partial) {
                      route += escapeString(token.prefix) + '(' + capture + ')?';
                    } else {
                      route += '(?:' + escapeString(token.prefix) + '(' + capture + '))?';
                    }
                  } else {
                    route += escapeString(token.prefix) + '(' + capture + ')';
                  }
                }
              }

              if (end) {
                if (!strict) route += '(?:' + delimiter + ')?';

                route += endsWith === '$' ? '$' : '(?=' + endsWith + ')';
              } else {
                if (!strict) route += '(?:' + delimiter + '(?=' + endsWith + '))?';
                if (!isEndDelimited) route += '(?=' + delimiter + '|' + endsWith + ')';
              }

              return new RegExp(route, flags(options))
            }

            /**
             * Normalize the given path string, returning a regular expression.
             *
             * An empty array can be passed in for the keys, which will hold the
             * placeholder key descriptions. For example, using `/user/:id`, `keys` will
             * contain `[{ name: 'id', delimiter: '/', optional: false, repeat: false }]`.
             *
             * @param  {(string|RegExp|Array)} path
             * @param  {Array=}                keys
             * @param  {Object=}               options
             * @return {!RegExp}
             */
            function pathToRegexp (path, keys, options) {
              if (path instanceof RegExp) {
                return regexpToRegexp(path, keys)
              }

              if (Array.isArray(path)) {
                return arrayToRegexp(/** @type {!Array} */ (path), keys, options)
              }

              return stringToRegexp(/** @type {string} */ (path), keys, options)
            }
            pathToRegexp_1.parse = parse_1;
            pathToRegexp_1.compile = compile_1;
            pathToRegexp_1.tokensToFunction = tokensToFunction_1;
            pathToRegexp_1.tokensToRegExp = tokensToRegExp_1;

            // Copyright Joyent, Inc. and other Node contributors.

            // If obj.hasOwnProperty has been overridden, then calling
            // obj.hasOwnProperty(prop) will break.
            // See: https://github.com/joyent/node/issues/1707
            function hasOwnProperty(obj, prop) {
              return Object.prototype.hasOwnProperty.call(obj, prop);
            }

            var decode = function(qs, sep, eq, options) {
              sep = sep || '&';
              eq = eq || '=';
              var obj = {};

              if (typeof qs !== 'string' || qs.length === 0) {
                return obj;
              }

              var regexp = /\+/g;
              qs = qs.split(sep);

              var maxKeys = 1000;
              if (options && typeof options.maxKeys === 'number') {
                maxKeys = options.maxKeys;
              }

              var len = qs.length;
              // maxKeys <= 0 means that we should not limit keys count
              if (maxKeys > 0 && len > maxKeys) {
                len = maxKeys;
              }

              for (var i = 0; i < len; ++i) {
                var x = qs[i].replace(regexp, '%20'),
                    idx = x.indexOf(eq),
                    kstr, vstr, k, v;

                if (idx >= 0) {
                  kstr = x.substr(0, idx);
                  vstr = x.substr(idx + 1);
                } else {
                  kstr = x;
                  vstr = '';
                }

                k = decodeURIComponent(kstr);
                v = decodeURIComponent(vstr);

                if (!hasOwnProperty(obj, k)) {
                  obj[k] = v;
                } else if (Array.isArray(obj[k])) {
                  obj[k].push(v);
                } else {
                  obj[k] = [obj[k], v];
                }
              }

              return obj;
            };

            // Copyright Joyent, Inc. and other Node contributors.

            var stringifyPrimitive = function(v) {
              switch (typeof v) {
                case 'string':
                  return v;

                case 'boolean':
                  return v ? 'true' : 'false';

                case 'number':
                  return isFinite(v) ? v : '';

                default:
                  return '';
              }
            };

            var encode = function(obj, sep, eq, name) {
              sep = sep || '&';
              eq = eq || '=';
              if (obj === null) {
                obj = undefined;
              }

              if (typeof obj === 'object') {
                return Object.keys(obj).map(function(k) {
                  var ks = encodeURIComponent(stringifyPrimitive(k)) + eq;
                  if (Array.isArray(obj[k])) {
                    return obj[k].map(function(v) {
                      return ks + encodeURIComponent(stringifyPrimitive(v));
                    }).join(sep);
                  } else {
                    return ks + encodeURIComponent(stringifyPrimitive(obj[k]));
                  }
                }).join(sep);

              }

              if (!name) return '';
              return encodeURIComponent(stringifyPrimitive(name)) + eq +
                     encodeURIComponent(stringifyPrimitive(obj));
            };

            var querystring = createCommonjsModule(function (module, exports) {

            exports.decode = exports.parse = decode;
            exports.encode = exports.stringify = encode;
            });
            var querystring_1 = querystring.decode;
            var querystring_2 = querystring.parse;
            var querystring_3 = querystring.encode;
            var querystring_4 = querystring.stringify;

            var isSubset_1 = createCommonjsModule(function (module, exports) {

            Object.defineProperty(exports, '__esModule', {
              value: true
            });
            /**
             * Check if an object is contained within another object.
             *
             * Returns `true` if:
             * - all enumerable keys of *subset* are also enumerable in *superset*, and
             * - every value assigned to an enumerable key of *subset* strictly equals
             *   the value assigned to the same key of *superset* – or is a subset of it.
             *
             * @param  {Object}  superset
             * @param  {Object}  subset
             *
             * @returns  {Boolean}
             *
             * @module    is-subset
             * @function  default
             * @alias     isSubset
             */
            var isSubset = (function (_isSubset) {
              function isSubset(_x, _x2) {
                return _isSubset.apply(this, arguments);
              }

              isSubset.toString = function () {
                return _isSubset.toString();
              };

              return isSubset;
            })(function (superset, subset) {
              if (typeof superset !== 'object' || superset === null || (typeof subset !== 'object' || subset === null)) return false;

              return Object.keys(subset).every(function (key) {
                if (!superset.propertyIsEnumerable(key)) return false;

                var subsetItem = subset[key];
                var supersetItem = superset[key];
                if (typeof subsetItem === 'object' && subsetItem !== null ? !isSubset(supersetItem, subsetItem) : supersetItem !== subsetItem) return false;

                return true;
              });
            });

            exports['default'] = isSubset;
            module.exports = exports['default'];
            });

            unwrapExports(isSubset_1);

            var lodash_isequal = createCommonjsModule(function (module, exports) {
            /**
             * Lodash (Custom Build) <https://lodash.com/>
             * Build: `lodash modularize exports="npm" -o ./`
             * Copyright JS Foundation and other contributors <https://js.foundation/>
             * Released under MIT license <https://lodash.com/license>
             * Based on Underscore.js 1.8.3 <http://underscorejs.org/LICENSE>
             * Copyright Jeremy Ashkenas, DocumentCloud and Investigative Reporters & Editors
             */

            /** Used as the size to enable large array optimizations. */
            var LARGE_ARRAY_SIZE = 200;

            /** Used to stand-in for `undefined` hash values. */
            var HASH_UNDEFINED = '__lodash_hash_undefined__';

            /** Used to compose bitmasks for value comparisons. */
            var COMPARE_PARTIAL_FLAG = 1,
                COMPARE_UNORDERED_FLAG = 2;

            /** Used as references for various `Number` constants. */
            var MAX_SAFE_INTEGER = 9007199254740991;

            /** `Object#toString` result references. */
            var argsTag = '[object Arguments]',
                arrayTag = '[object Array]',
                asyncTag = '[object AsyncFunction]',
                boolTag = '[object Boolean]',
                dateTag = '[object Date]',
                errorTag = '[object Error]',
                funcTag = '[object Function]',
                genTag = '[object GeneratorFunction]',
                mapTag = '[object Map]',
                numberTag = '[object Number]',
                nullTag = '[object Null]',
                objectTag = '[object Object]',
                promiseTag = '[object Promise]',
                proxyTag = '[object Proxy]',
                regexpTag = '[object RegExp]',
                setTag = '[object Set]',
                stringTag = '[object String]',
                symbolTag = '[object Symbol]',
                undefinedTag = '[object Undefined]',
                weakMapTag = '[object WeakMap]';

            var arrayBufferTag = '[object ArrayBuffer]',
                dataViewTag = '[object DataView]',
                float32Tag = '[object Float32Array]',
                float64Tag = '[object Float64Array]',
                int8Tag = '[object Int8Array]',
                int16Tag = '[object Int16Array]',
                int32Tag = '[object Int32Array]',
                uint8Tag = '[object Uint8Array]',
                uint8ClampedTag = '[object Uint8ClampedArray]',
                uint16Tag = '[object Uint16Array]',
                uint32Tag = '[object Uint32Array]';

            /**
             * Used to match `RegExp`
             * [syntax characters](http://ecma-international.org/ecma-262/7.0/#sec-patterns).
             */
            var reRegExpChar = /[\\^$.*+?()[\]{}|]/g;

            /** Used to detect host constructors (Safari). */
            var reIsHostCtor = /^\[object .+?Constructor\]$/;

            /** Used to detect unsigned integer values. */
            var reIsUint = /^(?:0|[1-9]\d*)$/;

            /** Used to identify `toStringTag` values of typed arrays. */
            var typedArrayTags = {};
            typedArrayTags[float32Tag] = typedArrayTags[float64Tag] =
            typedArrayTags[int8Tag] = typedArrayTags[int16Tag] =
            typedArrayTags[int32Tag] = typedArrayTags[uint8Tag] =
            typedArrayTags[uint8ClampedTag] = typedArrayTags[uint16Tag] =
            typedArrayTags[uint32Tag] = true;
            typedArrayTags[argsTag] = typedArrayTags[arrayTag] =
            typedArrayTags[arrayBufferTag] = typedArrayTags[boolTag] =
            typedArrayTags[dataViewTag] = typedArrayTags[dateTag] =
            typedArrayTags[errorTag] = typedArrayTags[funcTag] =
            typedArrayTags[mapTag] = typedArrayTags[numberTag] =
            typedArrayTags[objectTag] = typedArrayTags[regexpTag] =
            typedArrayTags[setTag] = typedArrayTags[stringTag] =
            typedArrayTags[weakMapTag] = false;

            /** Detect free variable `global` from Node.js. */
            var freeGlobal = typeof commonjsGlobal == 'object' && commonjsGlobal && commonjsGlobal.Object === Object && commonjsGlobal;

            /** Detect free variable `self`. */
            var freeSelf = typeof self == 'object' && self && self.Object === Object && self;

            /** Used as a reference to the global object. */
            var root = freeGlobal || freeSelf || Function('return this')();

            /** Detect free variable `exports`. */
            var freeExports =  exports && !exports.nodeType && exports;

            /** Detect free variable `module`. */
            var freeModule = freeExports && 'object' == 'object' && module && !module.nodeType && module;

            /** Detect the popular CommonJS extension `module.exports`. */
            var moduleExports = freeModule && freeModule.exports === freeExports;

            /** Detect free variable `process` from Node.js. */
            var freeProcess = moduleExports && freeGlobal.process;

            /** Used to access faster Node.js helpers. */
            var nodeUtil = (function() {
              try {
                return freeProcess && freeProcess.binding && freeProcess.binding('util');
              } catch (e) {}
            }());

            /* Node.js helper references. */
            var nodeIsTypedArray = nodeUtil && nodeUtil.isTypedArray;

            /**
             * A specialized version of `_.filter` for arrays without support for
             * iteratee shorthands.
             *
             * @private
             * @param {Array} [array] The array to iterate over.
             * @param {Function} predicate The function invoked per iteration.
             * @returns {Array} Returns the new filtered array.
             */
            function arrayFilter(array, predicate) {
              var index = -1,
                  length = array == null ? 0 : array.length,
                  resIndex = 0,
                  result = [];

              while (++index < length) {
                var value = array[index];
                if (predicate(value, index, array)) {
                  result[resIndex++] = value;
                }
              }
              return result;
            }

            /**
             * Appends the elements of `values` to `array`.
             *
             * @private
             * @param {Array} array The array to modify.
             * @param {Array} values The values to append.
             * @returns {Array} Returns `array`.
             */
            function arrayPush(array, values) {
              var index = -1,
                  length = values.length,
                  offset = array.length;

              while (++index < length) {
                array[offset + index] = values[index];
              }
              return array;
            }

            /**
             * A specialized version of `_.some` for arrays without support for iteratee
             * shorthands.
             *
             * @private
             * @param {Array} [array] The array to iterate over.
             * @param {Function} predicate The function invoked per iteration.
             * @returns {boolean} Returns `true` if any element passes the predicate check,
             *  else `false`.
             */
            function arraySome(array, predicate) {
              var index = -1,
                  length = array == null ? 0 : array.length;

              while (++index < length) {
                if (predicate(array[index], index, array)) {
                  return true;
                }
              }
              return false;
            }

            /**
             * The base implementation of `_.times` without support for iteratee shorthands
             * or max array length checks.
             *
             * @private
             * @param {number} n The number of times to invoke `iteratee`.
             * @param {Function} iteratee The function invoked per iteration.
             * @returns {Array} Returns the array of results.
             */
            function baseTimes(n, iteratee) {
              var index = -1,
                  result = Array(n);

              while (++index < n) {
                result[index] = iteratee(index);
              }
              return result;
            }

            /**
             * The base implementation of `_.unary` without support for storing metadata.
             *
             * @private
             * @param {Function} func The function to cap arguments for.
             * @returns {Function} Returns the new capped function.
             */
            function baseUnary(func) {
              return function(value) {
                return func(value);
              };
            }

            /**
             * Checks if a `cache` value for `key` exists.
             *
             * @private
             * @param {Object} cache The cache to query.
             * @param {string} key The key of the entry to check.
             * @returns {boolean} Returns `true` if an entry for `key` exists, else `false`.
             */
            function cacheHas(cache, key) {
              return cache.has(key);
            }

            /**
             * Gets the value at `key` of `object`.
             *
             * @private
             * @param {Object} [object] The object to query.
             * @param {string} key The key of the property to get.
             * @returns {*} Returns the property value.
             */
            function getValue(object, key) {
              return object == null ? undefined : object[key];
            }

            /**
             * Converts `map` to its key-value pairs.
             *
             * @private
             * @param {Object} map The map to convert.
             * @returns {Array} Returns the key-value pairs.
             */
            function mapToArray(map) {
              var index = -1,
                  result = Array(map.size);

              map.forEach(function(value, key) {
                result[++index] = [key, value];
              });
              return result;
            }

            /**
             * Creates a unary function that invokes `func` with its argument transformed.
             *
             * @private
             * @param {Function} func The function to wrap.
             * @param {Function} transform The argument transform.
             * @returns {Function} Returns the new function.
             */
            function overArg(func, transform) {
              return function(arg) {
                return func(transform(arg));
              };
            }

            /**
             * Converts `set` to an array of its values.
             *
             * @private
             * @param {Object} set The set to convert.
             * @returns {Array} Returns the values.
             */
            function setToArray(set) {
              var index = -1,
                  result = Array(set.size);

              set.forEach(function(value) {
                result[++index] = value;
              });
              return result;
            }

            /** Used for built-in method references. */
            var arrayProto = Array.prototype,
                funcProto = Function.prototype,
                objectProto = Object.prototype;

            /** Used to detect overreaching core-js shims. */
            var coreJsData = root['__core-js_shared__'];

            /** Used to resolve the decompiled source of functions. */
            var funcToString = funcProto.toString;

            /** Used to check objects for own properties. */
            var hasOwnProperty = objectProto.hasOwnProperty;

            /** Used to detect methods masquerading as native. */
            var maskSrcKey = (function() {
              var uid = /[^.]+$/.exec(coreJsData && coreJsData.keys && coreJsData.keys.IE_PROTO || '');
              return uid ? ('Symbol(src)_1.' + uid) : '';
            }());

            /**
             * Used to resolve the
             * [`toStringTag`](http://ecma-international.org/ecma-262/7.0/#sec-object.prototype.tostring)
             * of values.
             */
            var nativeObjectToString = objectProto.toString;

            /** Used to detect if a method is native. */
            var reIsNative = RegExp('^' +
              funcToString.call(hasOwnProperty).replace(reRegExpChar, '\\$&')
              .replace(/hasOwnProperty|(function).*?(?=\\\()| for .+?(?=\\\])/g, '$1.*?') + '$'
            );

            /** Built-in value references. */
            var Buffer = moduleExports ? root.Buffer : undefined,
                Symbol = root.Symbol,
                Uint8Array = root.Uint8Array,
                propertyIsEnumerable = objectProto.propertyIsEnumerable,
                splice = arrayProto.splice,
                symToStringTag = Symbol ? Symbol.toStringTag : undefined;

            /* Built-in method references for those with the same name as other `lodash` methods. */
            var nativeGetSymbols = Object.getOwnPropertySymbols,
                nativeIsBuffer = Buffer ? Buffer.isBuffer : undefined,
                nativeKeys = overArg(Object.keys, Object);

            /* Built-in method references that are verified to be native. */
            var DataView = getNative(root, 'DataView'),
                Map = getNative(root, 'Map'),
                Promise = getNative(root, 'Promise'),
                Set = getNative(root, 'Set'),
                WeakMap = getNative(root, 'WeakMap'),
                nativeCreate = getNative(Object, 'create');

            /** Used to detect maps, sets, and weakmaps. */
            var dataViewCtorString = toSource(DataView),
                mapCtorString = toSource(Map),
                promiseCtorString = toSource(Promise),
                setCtorString = toSource(Set),
                weakMapCtorString = toSource(WeakMap);

            /** Used to convert symbols to primitives and strings. */
            var symbolProto = Symbol ? Symbol.prototype : undefined,
                symbolValueOf = symbolProto ? symbolProto.valueOf : undefined;

            /**
             * Creates a hash object.
             *
             * @private
             * @constructor
             * @param {Array} [entries] The key-value pairs to cache.
             */
            function Hash(entries) {
              var index = -1,
                  length = entries == null ? 0 : entries.length;

              this.clear();
              while (++index < length) {
                var entry = entries[index];
                this.set(entry[0], entry[1]);
              }
            }

            /**
             * Removes all key-value entries from the hash.
             *
             * @private
             * @name clear
             * @memberOf Hash
             */
            function hashClear() {
              this.__data__ = nativeCreate ? nativeCreate(null) : {};
              this.size = 0;
            }

            /**
             * Removes `key` and its value from the hash.
             *
             * @private
             * @name delete
             * @memberOf Hash
             * @param {Object} hash The hash to modify.
             * @param {string} key The key of the value to remove.
             * @returns {boolean} Returns `true` if the entry was removed, else `false`.
             */
            function hashDelete(key) {
              var result = this.has(key) && delete this.__data__[key];
              this.size -= result ? 1 : 0;
              return result;
            }

            /**
             * Gets the hash value for `key`.
             *
             * @private
             * @name get
             * @memberOf Hash
             * @param {string} key The key of the value to get.
             * @returns {*} Returns the entry value.
             */
            function hashGet(key) {
              var data = this.__data__;
              if (nativeCreate) {
                var result = data[key];
                return result === HASH_UNDEFINED ? undefined : result;
              }
              return hasOwnProperty.call(data, key) ? data[key] : undefined;
            }

            /**
             * Checks if a hash value for `key` exists.
             *
             * @private
             * @name has
             * @memberOf Hash
             * @param {string} key The key of the entry to check.
             * @returns {boolean} Returns `true` if an entry for `key` exists, else `false`.
             */
            function hashHas(key) {
              var data = this.__data__;
              return nativeCreate ? (data[key] !== undefined) : hasOwnProperty.call(data, key);
            }

            /**
             * Sets the hash `key` to `value`.
             *
             * @private
             * @name set
             * @memberOf Hash
             * @param {string} key The key of the value to set.
             * @param {*} value The value to set.
             * @returns {Object} Returns the hash instance.
             */
            function hashSet(key, value) {
              var data = this.__data__;
              this.size += this.has(key) ? 0 : 1;
              data[key] = (nativeCreate && value === undefined) ? HASH_UNDEFINED : value;
              return this;
            }

            // Add methods to `Hash`.
            Hash.prototype.clear = hashClear;
            Hash.prototype['delete'] = hashDelete;
            Hash.prototype.get = hashGet;
            Hash.prototype.has = hashHas;
            Hash.prototype.set = hashSet;

            /**
             * Creates an list cache object.
             *
             * @private
             * @constructor
             * @param {Array} [entries] The key-value pairs to cache.
             */
            function ListCache(entries) {
              var index = -1,
                  length = entries == null ? 0 : entries.length;

              this.clear();
              while (++index < length) {
                var entry = entries[index];
                this.set(entry[0], entry[1]);
              }
            }

            /**
             * Removes all key-value entries from the list cache.
             *
             * @private
             * @name clear
             * @memberOf ListCache
             */
            function listCacheClear() {
              this.__data__ = [];
              this.size = 0;
            }

            /**
             * Removes `key` and its value from the list cache.
             *
             * @private
             * @name delete
             * @memberOf ListCache
             * @param {string} key The key of the value to remove.
             * @returns {boolean} Returns `true` if the entry was removed, else `false`.
             */
            function listCacheDelete(key) {
              var data = this.__data__,
                  index = assocIndexOf(data, key);

              if (index < 0) {
                return false;
              }
              var lastIndex = data.length - 1;
              if (index == lastIndex) {
                data.pop();
              } else {
                splice.call(data, index, 1);
              }
              --this.size;
              return true;
            }

            /**
             * Gets the list cache value for `key`.
             *
             * @private
             * @name get
             * @memberOf ListCache
             * @param {string} key The key of the value to get.
             * @returns {*} Returns the entry value.
             */
            function listCacheGet(key) {
              var data = this.__data__,
                  index = assocIndexOf(data, key);

              return index < 0 ? undefined : data[index][1];
            }

            /**
             * Checks if a list cache value for `key` exists.
             *
             * @private
             * @name has
             * @memberOf ListCache
             * @param {string} key The key of the entry to check.
             * @returns {boolean} Returns `true` if an entry for `key` exists, else `false`.
             */
            function listCacheHas(key) {
              return assocIndexOf(this.__data__, key) > -1;
            }

            /**
             * Sets the list cache `key` to `value`.
             *
             * @private
             * @name set
             * @memberOf ListCache
             * @param {string} key The key of the value to set.
             * @param {*} value The value to set.
             * @returns {Object} Returns the list cache instance.
             */
            function listCacheSet(key, value) {
              var data = this.__data__,
                  index = assocIndexOf(data, key);

              if (index < 0) {
                ++this.size;
                data.push([key, value]);
              } else {
                data[index][1] = value;
              }
              return this;
            }

            // Add methods to `ListCache`.
            ListCache.prototype.clear = listCacheClear;
            ListCache.prototype['delete'] = listCacheDelete;
            ListCache.prototype.get = listCacheGet;
            ListCache.prototype.has = listCacheHas;
            ListCache.prototype.set = listCacheSet;

            /**
             * Creates a map cache object to store key-value pairs.
             *
             * @private
             * @constructor
             * @param {Array} [entries] The key-value pairs to cache.
             */
            function MapCache(entries) {
              var index = -1,
                  length = entries == null ? 0 : entries.length;

              this.clear();
              while (++index < length) {
                var entry = entries[index];
                this.set(entry[0], entry[1]);
              }
            }

            /**
             * Removes all key-value entries from the map.
             *
             * @private
             * @name clear
             * @memberOf MapCache
             */
            function mapCacheClear() {
              this.size = 0;
              this.__data__ = {
                'hash': new Hash,
                'map': new (Map || ListCache),
                'string': new Hash
              };
            }

            /**
             * Removes `key` and its value from the map.
             *
             * @private
             * @name delete
             * @memberOf MapCache
             * @param {string} key The key of the value to remove.
             * @returns {boolean} Returns `true` if the entry was removed, else `false`.
             */
            function mapCacheDelete(key) {
              var result = getMapData(this, key)['delete'](key);
              this.size -= result ? 1 : 0;
              return result;
            }

            /**
             * Gets the map value for `key`.
             *
             * @private
             * @name get
             * @memberOf MapCache
             * @param {string} key The key of the value to get.
             * @returns {*} Returns the entry value.
             */
            function mapCacheGet(key) {
              return getMapData(this, key).get(key);
            }

            /**
             * Checks if a map value for `key` exists.
             *
             * @private
             * @name has
             * @memberOf MapCache
             * @param {string} key The key of the entry to check.
             * @returns {boolean} Returns `true` if an entry for `key` exists, else `false`.
             */
            function mapCacheHas(key) {
              return getMapData(this, key).has(key);
            }

            /**
             * Sets the map `key` to `value`.
             *
             * @private
             * @name set
             * @memberOf MapCache
             * @param {string} key The key of the value to set.
             * @param {*} value The value to set.
             * @returns {Object} Returns the map cache instance.
             */
            function mapCacheSet(key, value) {
              var data = getMapData(this, key),
                  size = data.size;

              data.set(key, value);
              this.size += data.size == size ? 0 : 1;
              return this;
            }

            // Add methods to `MapCache`.
            MapCache.prototype.clear = mapCacheClear;
            MapCache.prototype['delete'] = mapCacheDelete;
            MapCache.prototype.get = mapCacheGet;
            MapCache.prototype.has = mapCacheHas;
            MapCache.prototype.set = mapCacheSet;

            /**
             *
             * Creates an array cache object to store unique values.
             *
             * @private
             * @constructor
             * @param {Array} [values] The values to cache.
             */
            function SetCache(values) {
              var index = -1,
                  length = values == null ? 0 : values.length;

              this.__data__ = new MapCache;
              while (++index < length) {
                this.add(values[index]);
              }
            }

            /**
             * Adds `value` to the array cache.
             *
             * @private
             * @name add
             * @memberOf SetCache
             * @alias push
             * @param {*} value The value to cache.
             * @returns {Object} Returns the cache instance.
             */
            function setCacheAdd(value) {
              this.__data__.set(value, HASH_UNDEFINED);
              return this;
            }

            /**
             * Checks if `value` is in the array cache.
             *
             * @private
             * @name has
             * @memberOf SetCache
             * @param {*} value The value to search for.
             * @returns {number} Returns `true` if `value` is found, else `false`.
             */
            function setCacheHas(value) {
              return this.__data__.has(value);
            }

            // Add methods to `SetCache`.
            SetCache.prototype.add = SetCache.prototype.push = setCacheAdd;
            SetCache.prototype.has = setCacheHas;

            /**
             * Creates a stack cache object to store key-value pairs.
             *
             * @private
             * @constructor
             * @param {Array} [entries] The key-value pairs to cache.
             */
            function Stack(entries) {
              var data = this.__data__ = new ListCache(entries);
              this.size = data.size;
            }

            /**
             * Removes all key-value entries from the stack.
             *
             * @private
             * @name clear
             * @memberOf Stack
             */
            function stackClear() {
              this.__data__ = new ListCache;
              this.size = 0;
            }

            /**
             * Removes `key` and its value from the stack.
             *
             * @private
             * @name delete
             * @memberOf Stack
             * @param {string} key The key of the value to remove.
             * @returns {boolean} Returns `true` if the entry was removed, else `false`.
             */
            function stackDelete(key) {
              var data = this.__data__,
                  result = data['delete'](key);

              this.size = data.size;
              return result;
            }

            /**
             * Gets the stack value for `key`.
             *
             * @private
             * @name get
             * @memberOf Stack
             * @param {string} key The key of the value to get.
             * @returns {*} Returns the entry value.
             */
            function stackGet(key) {
              return this.__data__.get(key);
            }

            /**
             * Checks if a stack value for `key` exists.
             *
             * @private
             * @name has
             * @memberOf Stack
             * @param {string} key The key of the entry to check.
             * @returns {boolean} Returns `true` if an entry for `key` exists, else `false`.
             */
            function stackHas(key) {
              return this.__data__.has(key);
            }

            /**
             * Sets the stack `key` to `value`.
             *
             * @private
             * @name set
             * @memberOf Stack
             * @param {string} key The key of the value to set.
             * @param {*} value The value to set.
             * @returns {Object} Returns the stack cache instance.
             */
            function stackSet(key, value) {
              var data = this.__data__;
              if (data instanceof ListCache) {
                var pairs = data.__data__;
                if (!Map || (pairs.length < LARGE_ARRAY_SIZE - 1)) {
                  pairs.push([key, value]);
                  this.size = ++data.size;
                  return this;
                }
                data = this.__data__ = new MapCache(pairs);
              }
              data.set(key, value);
              this.size = data.size;
              return this;
            }

            // Add methods to `Stack`.
            Stack.prototype.clear = stackClear;
            Stack.prototype['delete'] = stackDelete;
            Stack.prototype.get = stackGet;
            Stack.prototype.has = stackHas;
            Stack.prototype.set = stackSet;

            /**
             * Creates an array of the enumerable property names of the array-like `value`.
             *
             * @private
             * @param {*} value The value to query.
             * @param {boolean} inherited Specify returning inherited property names.
             * @returns {Array} Returns the array of property names.
             */
            function arrayLikeKeys(value, inherited) {
              var isArr = isArray(value),
                  isArg = !isArr && isArguments(value),
                  isBuff = !isArr && !isArg && isBuffer(value),
                  isType = !isArr && !isArg && !isBuff && isTypedArray(value),
                  skipIndexes = isArr || isArg || isBuff || isType,
                  result = skipIndexes ? baseTimes(value.length, String) : [],
                  length = result.length;

              for (var key in value) {
                if ((inherited || hasOwnProperty.call(value, key)) &&
                    !(skipIndexes && (
                       // Safari 9 has enumerable `arguments.length` in strict mode.
                       key == 'length' ||
                       // Node.js 0.10 has enumerable non-index properties on buffers.
                       (isBuff && (key == 'offset' || key == 'parent')) ||
                       // PhantomJS 2 has enumerable non-index properties on typed arrays.
                       (isType && (key == 'buffer' || key == 'byteLength' || key == 'byteOffset')) ||
                       // Skip index properties.
                       isIndex(key, length)
                    ))) {
                  result.push(key);
                }
              }
              return result;
            }

            /**
             * Gets the index at which the `key` is found in `array` of key-value pairs.
             *
             * @private
             * @param {Array} array The array to inspect.
             * @param {*} key The key to search for.
             * @returns {number} Returns the index of the matched value, else `-1`.
             */
            function assocIndexOf(array, key) {
              var length = array.length;
              while (length--) {
                if (eq(array[length][0], key)) {
                  return length;
                }
              }
              return -1;
            }

            /**
             * The base implementation of `getAllKeys` and `getAllKeysIn` which uses
             * `keysFunc` and `symbolsFunc` to get the enumerable property names and
             * symbols of `object`.
             *
             * @private
             * @param {Object} object The object to query.
             * @param {Function} keysFunc The function to get the keys of `object`.
             * @param {Function} symbolsFunc The function to get the symbols of `object`.
             * @returns {Array} Returns the array of property names and symbols.
             */
            function baseGetAllKeys(object, keysFunc, symbolsFunc) {
              var result = keysFunc(object);
              return isArray(object) ? result : arrayPush(result, symbolsFunc(object));
            }

            /**
             * The base implementation of `getTag` without fallbacks for buggy environments.
             *
             * @private
             * @param {*} value The value to query.
             * @returns {string} Returns the `toStringTag`.
             */
            function baseGetTag(value) {
              if (value == null) {
                return value === undefined ? undefinedTag : nullTag;
              }
              return (symToStringTag && symToStringTag in Object(value))
                ? getRawTag(value)
                : objectToString(value);
            }

            /**
             * The base implementation of `_.isArguments`.
             *
             * @private
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is an `arguments` object,
             */
            function baseIsArguments(value) {
              return isObjectLike(value) && baseGetTag(value) == argsTag;
            }

            /**
             * The base implementation of `_.isEqual` which supports partial comparisons
             * and tracks traversed objects.
             *
             * @private
             * @param {*} value The value to compare.
             * @param {*} other The other value to compare.
             * @param {boolean} bitmask The bitmask flags.
             *  1 - Unordered comparison
             *  2 - Partial comparison
             * @param {Function} [customizer] The function to customize comparisons.
             * @param {Object} [stack] Tracks traversed `value` and `other` objects.
             * @returns {boolean} Returns `true` if the values are equivalent, else `false`.
             */
            function baseIsEqual(value, other, bitmask, customizer, stack) {
              if (value === other) {
                return true;
              }
              if (value == null || other == null || (!isObjectLike(value) && !isObjectLike(other))) {
                return value !== value && other !== other;
              }
              return baseIsEqualDeep(value, other, bitmask, customizer, baseIsEqual, stack);
            }

            /**
             * A specialized version of `baseIsEqual` for arrays and objects which performs
             * deep comparisons and tracks traversed objects enabling objects with circular
             * references to be compared.
             *
             * @private
             * @param {Object} object The object to compare.
             * @param {Object} other The other object to compare.
             * @param {number} bitmask The bitmask flags. See `baseIsEqual` for more details.
             * @param {Function} customizer The function to customize comparisons.
             * @param {Function} equalFunc The function to determine equivalents of values.
             * @param {Object} [stack] Tracks traversed `object` and `other` objects.
             * @returns {boolean} Returns `true` if the objects are equivalent, else `false`.
             */
            function baseIsEqualDeep(object, other, bitmask, customizer, equalFunc, stack) {
              var objIsArr = isArray(object),
                  othIsArr = isArray(other),
                  objTag = objIsArr ? arrayTag : getTag(object),
                  othTag = othIsArr ? arrayTag : getTag(other);

              objTag = objTag == argsTag ? objectTag : objTag;
              othTag = othTag == argsTag ? objectTag : othTag;

              var objIsObj = objTag == objectTag,
                  othIsObj = othTag == objectTag,
                  isSameTag = objTag == othTag;

              if (isSameTag && isBuffer(object)) {
                if (!isBuffer(other)) {
                  return false;
                }
                objIsArr = true;
                objIsObj = false;
              }
              if (isSameTag && !objIsObj) {
                stack || (stack = new Stack);
                return (objIsArr || isTypedArray(object))
                  ? equalArrays(object, other, bitmask, customizer, equalFunc, stack)
                  : equalByTag(object, other, objTag, bitmask, customizer, equalFunc, stack);
              }
              if (!(bitmask & COMPARE_PARTIAL_FLAG)) {
                var objIsWrapped = objIsObj && hasOwnProperty.call(object, '__wrapped__'),
                    othIsWrapped = othIsObj && hasOwnProperty.call(other, '__wrapped__');

                if (objIsWrapped || othIsWrapped) {
                  var objUnwrapped = objIsWrapped ? object.value() : object,
                      othUnwrapped = othIsWrapped ? other.value() : other;

                  stack || (stack = new Stack);
                  return equalFunc(objUnwrapped, othUnwrapped, bitmask, customizer, stack);
                }
              }
              if (!isSameTag) {
                return false;
              }
              stack || (stack = new Stack);
              return equalObjects(object, other, bitmask, customizer, equalFunc, stack);
            }

            /**
             * The base implementation of `_.isNative` without bad shim checks.
             *
             * @private
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is a native function,
             *  else `false`.
             */
            function baseIsNative(value) {
              if (!isObject(value) || isMasked(value)) {
                return false;
              }
              var pattern = isFunction(value) ? reIsNative : reIsHostCtor;
              return pattern.test(toSource(value));
            }

            /**
             * The base implementation of `_.isTypedArray` without Node.js optimizations.
             *
             * @private
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is a typed array, else `false`.
             */
            function baseIsTypedArray(value) {
              return isObjectLike(value) &&
                isLength(value.length) && !!typedArrayTags[baseGetTag(value)];
            }

            /**
             * The base implementation of `_.keys` which doesn't treat sparse arrays as dense.
             *
             * @private
             * @param {Object} object The object to query.
             * @returns {Array} Returns the array of property names.
             */
            function baseKeys(object) {
              if (!isPrototype(object)) {
                return nativeKeys(object);
              }
              var result = [];
              for (var key in Object(object)) {
                if (hasOwnProperty.call(object, key) && key != 'constructor') {
                  result.push(key);
                }
              }
              return result;
            }

            /**
             * A specialized version of `baseIsEqualDeep` for arrays with support for
             * partial deep comparisons.
             *
             * @private
             * @param {Array} array The array to compare.
             * @param {Array} other The other array to compare.
             * @param {number} bitmask The bitmask flags. See `baseIsEqual` for more details.
             * @param {Function} customizer The function to customize comparisons.
             * @param {Function} equalFunc The function to determine equivalents of values.
             * @param {Object} stack Tracks traversed `array` and `other` objects.
             * @returns {boolean} Returns `true` if the arrays are equivalent, else `false`.
             */
            function equalArrays(array, other, bitmask, customizer, equalFunc, stack) {
              var isPartial = bitmask & COMPARE_PARTIAL_FLAG,
                  arrLength = array.length,
                  othLength = other.length;

              if (arrLength != othLength && !(isPartial && othLength > arrLength)) {
                return false;
              }
              // Assume cyclic values are equal.
              var stacked = stack.get(array);
              if (stacked && stack.get(other)) {
                return stacked == other;
              }
              var index = -1,
                  result = true,
                  seen = (bitmask & COMPARE_UNORDERED_FLAG) ? new SetCache : undefined;

              stack.set(array, other);
              stack.set(other, array);

              // Ignore non-index properties.
              while (++index < arrLength) {
                var arrValue = array[index],
                    othValue = other[index];

                if (customizer) {
                  var compared = isPartial
                    ? customizer(othValue, arrValue, index, other, array, stack)
                    : customizer(arrValue, othValue, index, array, other, stack);
                }
                if (compared !== undefined) {
                  if (compared) {
                    continue;
                  }
                  result = false;
                  break;
                }
                // Recursively compare arrays (susceptible to call stack limits).
                if (seen) {
                  if (!arraySome(other, function(othValue, othIndex) {
                        if (!cacheHas(seen, othIndex) &&
                            (arrValue === othValue || equalFunc(arrValue, othValue, bitmask, customizer, stack))) {
                          return seen.push(othIndex);
                        }
                      })) {
                    result = false;
                    break;
                  }
                } else if (!(
                      arrValue === othValue ||
                        equalFunc(arrValue, othValue, bitmask, customizer, stack)
                    )) {
                  result = false;
                  break;
                }
              }
              stack['delete'](array);
              stack['delete'](other);
              return result;
            }

            /**
             * A specialized version of `baseIsEqualDeep` for comparing objects of
             * the same `toStringTag`.
             *
             * **Note:** This function only supports comparing values with tags of
             * `Boolean`, `Date`, `Error`, `Number`, `RegExp`, or `String`.
             *
             * @private
             * @param {Object} object The object to compare.
             * @param {Object} other The other object to compare.
             * @param {string} tag The `toStringTag` of the objects to compare.
             * @param {number} bitmask The bitmask flags. See `baseIsEqual` for more details.
             * @param {Function} customizer The function to customize comparisons.
             * @param {Function} equalFunc The function to determine equivalents of values.
             * @param {Object} stack Tracks traversed `object` and `other` objects.
             * @returns {boolean} Returns `true` if the objects are equivalent, else `false`.
             */
            function equalByTag(object, other, tag, bitmask, customizer, equalFunc, stack) {
              switch (tag) {
                case dataViewTag:
                  if ((object.byteLength != other.byteLength) ||
                      (object.byteOffset != other.byteOffset)) {
                    return false;
                  }
                  object = object.buffer;
                  other = other.buffer;

                case arrayBufferTag:
                  if ((object.byteLength != other.byteLength) ||
                      !equalFunc(new Uint8Array(object), new Uint8Array(other))) {
                    return false;
                  }
                  return true;

                case boolTag:
                case dateTag:
                case numberTag:
                  // Coerce booleans to `1` or `0` and dates to milliseconds.
                  // Invalid dates are coerced to `NaN`.
                  return eq(+object, +other);

                case errorTag:
                  return object.name == other.name && object.message == other.message;

                case regexpTag:
                case stringTag:
                  // Coerce regexes to strings and treat strings, primitives and objects,
                  // as equal. See http://www.ecma-international.org/ecma-262/7.0/#sec-regexp.prototype.tostring
                  // for more details.
                  return object == (other + '');

                case mapTag:
                  var convert = mapToArray;

                case setTag:
                  var isPartial = bitmask & COMPARE_PARTIAL_FLAG;
                  convert || (convert = setToArray);

                  if (object.size != other.size && !isPartial) {
                    return false;
                  }
                  // Assume cyclic values are equal.
                  var stacked = stack.get(object);
                  if (stacked) {
                    return stacked == other;
                  }
                  bitmask |= COMPARE_UNORDERED_FLAG;

                  // Recursively compare objects (susceptible to call stack limits).
                  stack.set(object, other);
                  var result = equalArrays(convert(object), convert(other), bitmask, customizer, equalFunc, stack);
                  stack['delete'](object);
                  return result;

                case symbolTag:
                  if (symbolValueOf) {
                    return symbolValueOf.call(object) == symbolValueOf.call(other);
                  }
              }
              return false;
            }

            /**
             * A specialized version of `baseIsEqualDeep` for objects with support for
             * partial deep comparisons.
             *
             * @private
             * @param {Object} object The object to compare.
             * @param {Object} other The other object to compare.
             * @param {number} bitmask The bitmask flags. See `baseIsEqual` for more details.
             * @param {Function} customizer The function to customize comparisons.
             * @param {Function} equalFunc The function to determine equivalents of values.
             * @param {Object} stack Tracks traversed `object` and `other` objects.
             * @returns {boolean} Returns `true` if the objects are equivalent, else `false`.
             */
            function equalObjects(object, other, bitmask, customizer, equalFunc, stack) {
              var isPartial = bitmask & COMPARE_PARTIAL_FLAG,
                  objProps = getAllKeys(object),
                  objLength = objProps.length,
                  othProps = getAllKeys(other),
                  othLength = othProps.length;

              if (objLength != othLength && !isPartial) {
                return false;
              }
              var index = objLength;
              while (index--) {
                var key = objProps[index];
                if (!(isPartial ? key in other : hasOwnProperty.call(other, key))) {
                  return false;
                }
              }
              // Assume cyclic values are equal.
              var stacked = stack.get(object);
              if (stacked && stack.get(other)) {
                return stacked == other;
              }
              var result = true;
              stack.set(object, other);
              stack.set(other, object);

              var skipCtor = isPartial;
              while (++index < objLength) {
                key = objProps[index];
                var objValue = object[key],
                    othValue = other[key];

                if (customizer) {
                  var compared = isPartial
                    ? customizer(othValue, objValue, key, other, object, stack)
                    : customizer(objValue, othValue, key, object, other, stack);
                }
                // Recursively compare objects (susceptible to call stack limits).
                if (!(compared === undefined
                      ? (objValue === othValue || equalFunc(objValue, othValue, bitmask, customizer, stack))
                      : compared
                    )) {
                  result = false;
                  break;
                }
                skipCtor || (skipCtor = key == 'constructor');
              }
              if (result && !skipCtor) {
                var objCtor = object.constructor,
                    othCtor = other.constructor;

                // Non `Object` object instances with different constructors are not equal.
                if (objCtor != othCtor &&
                    ('constructor' in object && 'constructor' in other) &&
                    !(typeof objCtor == 'function' && objCtor instanceof objCtor &&
                      typeof othCtor == 'function' && othCtor instanceof othCtor)) {
                  result = false;
                }
              }
              stack['delete'](object);
              stack['delete'](other);
              return result;
            }

            /**
             * Creates an array of own enumerable property names and symbols of `object`.
             *
             * @private
             * @param {Object} object The object to query.
             * @returns {Array} Returns the array of property names and symbols.
             */
            function getAllKeys(object) {
              return baseGetAllKeys(object, keys, getSymbols);
            }

            /**
             * Gets the data for `map`.
             *
             * @private
             * @param {Object} map The map to query.
             * @param {string} key The reference key.
             * @returns {*} Returns the map data.
             */
            function getMapData(map, key) {
              var data = map.__data__;
              return isKeyable(key)
                ? data[typeof key == 'string' ? 'string' : 'hash']
                : data.map;
            }

            /**
             * Gets the native function at `key` of `object`.
             *
             * @private
             * @param {Object} object The object to query.
             * @param {string} key The key of the method to get.
             * @returns {*} Returns the function if it's native, else `undefined`.
             */
            function getNative(object, key) {
              var value = getValue(object, key);
              return baseIsNative(value) ? value : undefined;
            }

            /**
             * A specialized version of `baseGetTag` which ignores `Symbol.toStringTag` values.
             *
             * @private
             * @param {*} value The value to query.
             * @returns {string} Returns the raw `toStringTag`.
             */
            function getRawTag(value) {
              var isOwn = hasOwnProperty.call(value, symToStringTag),
                  tag = value[symToStringTag];

              try {
                value[symToStringTag] = undefined;
                var unmasked = true;
              } catch (e) {}

              var result = nativeObjectToString.call(value);
              if (unmasked) {
                if (isOwn) {
                  value[symToStringTag] = tag;
                } else {
                  delete value[symToStringTag];
                }
              }
              return result;
            }

            /**
             * Creates an array of the own enumerable symbols of `object`.
             *
             * @private
             * @param {Object} object The object to query.
             * @returns {Array} Returns the array of symbols.
             */
            var getSymbols = !nativeGetSymbols ? stubArray : function(object) {
              if (object == null) {
                return [];
              }
              object = Object(object);
              return arrayFilter(nativeGetSymbols(object), function(symbol) {
                return propertyIsEnumerable.call(object, symbol);
              });
            };

            /**
             * Gets the `toStringTag` of `value`.
             *
             * @private
             * @param {*} value The value to query.
             * @returns {string} Returns the `toStringTag`.
             */
            var getTag = baseGetTag;

            // Fallback for data views, maps, sets, and weak maps in IE 11 and promises in Node.js < 6.
            if ((DataView && getTag(new DataView(new ArrayBuffer(1))) != dataViewTag) ||
                (Map && getTag(new Map) != mapTag) ||
                (Promise && getTag(Promise.resolve()) != promiseTag) ||
                (Set && getTag(new Set) != setTag) ||
                (WeakMap && getTag(new WeakMap) != weakMapTag)) {
              getTag = function(value) {
                var result = baseGetTag(value),
                    Ctor = result == objectTag ? value.constructor : undefined,
                    ctorString = Ctor ? toSource(Ctor) : '';

                if (ctorString) {
                  switch (ctorString) {
                    case dataViewCtorString: return dataViewTag;
                    case mapCtorString: return mapTag;
                    case promiseCtorString: return promiseTag;
                    case setCtorString: return setTag;
                    case weakMapCtorString: return weakMapTag;
                  }
                }
                return result;
              };
            }

            /**
             * Checks if `value` is a valid array-like index.
             *
             * @private
             * @param {*} value The value to check.
             * @param {number} [length=MAX_SAFE_INTEGER] The upper bounds of a valid index.
             * @returns {boolean} Returns `true` if `value` is a valid index, else `false`.
             */
            function isIndex(value, length) {
              length = length == null ? MAX_SAFE_INTEGER : length;
              return !!length &&
                (typeof value == 'number' || reIsUint.test(value)) &&
                (value > -1 && value % 1 == 0 && value < length);
            }

            /**
             * Checks if `value` is suitable for use as unique object key.
             *
             * @private
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is suitable, else `false`.
             */
            function isKeyable(value) {
              var type = typeof value;
              return (type == 'string' || type == 'number' || type == 'symbol' || type == 'boolean')
                ? (value !== '__proto__')
                : (value === null);
            }

            /**
             * Checks if `func` has its source masked.
             *
             * @private
             * @param {Function} func The function to check.
             * @returns {boolean} Returns `true` if `func` is masked, else `false`.
             */
            function isMasked(func) {
              return !!maskSrcKey && (maskSrcKey in func);
            }

            /**
             * Checks if `value` is likely a prototype object.
             *
             * @private
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is a prototype, else `false`.
             */
            function isPrototype(value) {
              var Ctor = value && value.constructor,
                  proto = (typeof Ctor == 'function' && Ctor.prototype) || objectProto;

              return value === proto;
            }

            /**
             * Converts `value` to a string using `Object.prototype.toString`.
             *
             * @private
             * @param {*} value The value to convert.
             * @returns {string} Returns the converted string.
             */
            function objectToString(value) {
              return nativeObjectToString.call(value);
            }

            /**
             * Converts `func` to its source code.
             *
             * @private
             * @param {Function} func The function to convert.
             * @returns {string} Returns the source code.
             */
            function toSource(func) {
              if (func != null) {
                try {
                  return funcToString.call(func);
                } catch (e) {}
                try {
                  return (func + '');
                } catch (e) {}
              }
              return '';
            }

            /**
             * Performs a
             * [`SameValueZero`](http://ecma-international.org/ecma-262/7.0/#sec-samevaluezero)
             * comparison between two values to determine if they are equivalent.
             *
             * @static
             * @memberOf _
             * @since 4.0.0
             * @category Lang
             * @param {*} value The value to compare.
             * @param {*} other The other value to compare.
             * @returns {boolean} Returns `true` if the values are equivalent, else `false`.
             * @example
             *
             * var object = { 'a': 1 };
             * var other = { 'a': 1 };
             *
             * _.eq(object, object);
             * // => true
             *
             * _.eq(object, other);
             * // => false
             *
             * _.eq('a', 'a');
             * // => true
             *
             * _.eq('a', Object('a'));
             * // => false
             *
             * _.eq(NaN, NaN);
             * // => true
             */
            function eq(value, other) {
              return value === other || (value !== value && other !== other);
            }

            /**
             * Checks if `value` is likely an `arguments` object.
             *
             * @static
             * @memberOf _
             * @since 0.1.0
             * @category Lang
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is an `arguments` object,
             *  else `false`.
             * @example
             *
             * _.isArguments(function() { return arguments; }());
             * // => true
             *
             * _.isArguments([1, 2, 3]);
             * // => false
             */
            var isArguments = baseIsArguments(function() { return arguments; }()) ? baseIsArguments : function(value) {
              return isObjectLike(value) && hasOwnProperty.call(value, 'callee') &&
                !propertyIsEnumerable.call(value, 'callee');
            };

            /**
             * Checks if `value` is classified as an `Array` object.
             *
             * @static
             * @memberOf _
             * @since 0.1.0
             * @category Lang
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is an array, else `false`.
             * @example
             *
             * _.isArray([1, 2, 3]);
             * // => true
             *
             * _.isArray(document.body.children);
             * // => false
             *
             * _.isArray('abc');
             * // => false
             *
             * _.isArray(_.noop);
             * // => false
             */
            var isArray = Array.isArray;

            /**
             * Checks if `value` is array-like. A value is considered array-like if it's
             * not a function and has a `value.length` that's an integer greater than or
             * equal to `0` and less than or equal to `Number.MAX_SAFE_INTEGER`.
             *
             * @static
             * @memberOf _
             * @since 4.0.0
             * @category Lang
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is array-like, else `false`.
             * @example
             *
             * _.isArrayLike([1, 2, 3]);
             * // => true
             *
             * _.isArrayLike(document.body.children);
             * // => true
             *
             * _.isArrayLike('abc');
             * // => true
             *
             * _.isArrayLike(_.noop);
             * // => false
             */
            function isArrayLike(value) {
              return value != null && isLength(value.length) && !isFunction(value);
            }

            /**
             * Checks if `value` is a buffer.
             *
             * @static
             * @memberOf _
             * @since 4.3.0
             * @category Lang
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is a buffer, else `false`.
             * @example
             *
             * _.isBuffer(new Buffer(2));
             * // => true
             *
             * _.isBuffer(new Uint8Array(2));
             * // => false
             */
            var isBuffer = nativeIsBuffer || stubFalse;

            /**
             * Performs a deep comparison between two values to determine if they are
             * equivalent.
             *
             * **Note:** This method supports comparing arrays, array buffers, booleans,
             * date objects, error objects, maps, numbers, `Object` objects, regexes,
             * sets, strings, symbols, and typed arrays. `Object` objects are compared
             * by their own, not inherited, enumerable properties. Functions and DOM
             * nodes are compared by strict equality, i.e. `===`.
             *
             * @static
             * @memberOf _
             * @since 0.1.0
             * @category Lang
             * @param {*} value The value to compare.
             * @param {*} other The other value to compare.
             * @returns {boolean} Returns `true` if the values are equivalent, else `false`.
             * @example
             *
             * var object = { 'a': 1 };
             * var other = { 'a': 1 };
             *
             * _.isEqual(object, other);
             * // => true
             *
             * object === other;
             * // => false
             */
            function isEqual(value, other) {
              return baseIsEqual(value, other);
            }

            /**
             * Checks if `value` is classified as a `Function` object.
             *
             * @static
             * @memberOf _
             * @since 0.1.0
             * @category Lang
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is a function, else `false`.
             * @example
             *
             * _.isFunction(_);
             * // => true
             *
             * _.isFunction(/abc/);
             * // => false
             */
            function isFunction(value) {
              if (!isObject(value)) {
                return false;
              }
              // The use of `Object#toString` avoids issues with the `typeof` operator
              // in Safari 9 which returns 'object' for typed arrays and other constructors.
              var tag = baseGetTag(value);
              return tag == funcTag || tag == genTag || tag == asyncTag || tag == proxyTag;
            }

            /**
             * Checks if `value` is a valid array-like length.
             *
             * **Note:** This method is loosely based on
             * [`ToLength`](http://ecma-international.org/ecma-262/7.0/#sec-tolength).
             *
             * @static
             * @memberOf _
             * @since 4.0.0
             * @category Lang
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is a valid length, else `false`.
             * @example
             *
             * _.isLength(3);
             * // => true
             *
             * _.isLength(Number.MIN_VALUE);
             * // => false
             *
             * _.isLength(Infinity);
             * // => false
             *
             * _.isLength('3');
             * // => false
             */
            function isLength(value) {
              return typeof value == 'number' &&
                value > -1 && value % 1 == 0 && value <= MAX_SAFE_INTEGER;
            }

            /**
             * Checks if `value` is the
             * [language type](http://www.ecma-international.org/ecma-262/7.0/#sec-ecmascript-language-types)
             * of `Object`. (e.g. arrays, functions, objects, regexes, `new Number(0)`, and `new String('')`)
             *
             * @static
             * @memberOf _
             * @since 0.1.0
             * @category Lang
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is an object, else `false`.
             * @example
             *
             * _.isObject({});
             * // => true
             *
             * _.isObject([1, 2, 3]);
             * // => true
             *
             * _.isObject(_.noop);
             * // => true
             *
             * _.isObject(null);
             * // => false
             */
            function isObject(value) {
              var type = typeof value;
              return value != null && (type == 'object' || type == 'function');
            }

            /**
             * Checks if `value` is object-like. A value is object-like if it's not `null`
             * and has a `typeof` result of "object".
             *
             * @static
             * @memberOf _
             * @since 4.0.0
             * @category Lang
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is object-like, else `false`.
             * @example
             *
             * _.isObjectLike({});
             * // => true
             *
             * _.isObjectLike([1, 2, 3]);
             * // => true
             *
             * _.isObjectLike(_.noop);
             * // => false
             *
             * _.isObjectLike(null);
             * // => false
             */
            function isObjectLike(value) {
              return value != null && typeof value == 'object';
            }

            /**
             * Checks if `value` is classified as a typed array.
             *
             * @static
             * @memberOf _
             * @since 3.0.0
             * @category Lang
             * @param {*} value The value to check.
             * @returns {boolean} Returns `true` if `value` is a typed array, else `false`.
             * @example
             *
             * _.isTypedArray(new Uint8Array);
             * // => true
             *
             * _.isTypedArray([]);
             * // => false
             */
            var isTypedArray = nodeIsTypedArray ? baseUnary(nodeIsTypedArray) : baseIsTypedArray;

            /**
             * Creates an array of the own enumerable property names of `object`.
             *
             * **Note:** Non-object values are coerced to objects. See the
             * [ES spec](http://ecma-international.org/ecma-262/7.0/#sec-object.keys)
             * for more details.
             *
             * @static
             * @since 0.1.0
             * @memberOf _
             * @category Object
             * @param {Object} object The object to query.
             * @returns {Array} Returns the array of property names.
             * @example
             *
             * function Foo() {
             *   this.a = 1;
             *   this.b = 2;
             * }
             *
             * Foo.prototype.c = 3;
             *
             * _.keys(new Foo);
             * // => ['a', 'b'] (iteration order is not guaranteed)
             *
             * _.keys('hi');
             * // => ['0', '1']
             */
            function keys(object) {
              return isArrayLike(object) ? arrayLikeKeys(object) : baseKeys(object);
            }

            /**
             * This method returns a new empty array.
             *
             * @static
             * @memberOf _
             * @since 4.13.0
             * @category Util
             * @returns {Array} Returns the new empty array.
             * @example
             *
             * var arrays = _.times(2, _.stubArray);
             *
             * console.log(arrays);
             * // => [[], []]
             *
             * console.log(arrays[0] === arrays[1]);
             * // => false
             */
            function stubArray() {
              return [];
            }

            /**
             * This method returns `false`.
             *
             * @static
             * @memberOf _
             * @since 4.13.0
             * @category Util
             * @returns {boolean} Returns `false`.
             * @example
             *
             * _.times(2, _.stubFalse);
             * // => [false, false]
             */
            function stubFalse() {
              return false;
            }

            module.exports = isEqual;
            });

            var _defineProperty2$1 = interopRequireDefault(defineProperty);

            var debug$2 = debug_1.debug;









            var headerUtils = requestUtils.headers,
                getPath = requestUtils.getPath,
                getQuery = requestUtils.getQuery,
                normalizeUrl$1 = requestUtils.normalizeUrl;



            var debuggableUrlFunc = function debuggableUrlFunc(func) {
              return function (url) {
                debug$2('Actual url:', url);
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
                var urlRX = globToRegexp(targetString);

                return debuggableUrlFunc(function (url) {
                  return urlRX.test(url);
                });
              },
              express: function express(targetString) {
                var urlRX = pathToRegexp_1(targetString);
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
              debug$2('Generating header matcher');

              if (!expectedHeaders) {
                debug$2('  No header expectations defined - skipping');
                return;
              }

              var expectation = headerUtils.toLowerCase(expectedHeaders);
              debug$2('  Expected headers:', expectation);
              return function (url, _ref2) {
                var _ref2$headers = _ref2.headers,
                    headers = _ref2$headers === void 0 ? {} : _ref2$headers;
                debug$2('Attempting to match headers');
                var lowerCaseHeaders = headerUtils.toLowerCase(headerUtils.normalize(headers));
                debug$2('  Expected headers:', expectation);
                debug$2('  Actual headers:', lowerCaseHeaders);
                return Object.keys(expectation).every(function (headerName) {
                  return headerUtils.equal(lowerCaseHeaders[headerName], expectation[headerName]);
                });
              };
            };

            var getMethodMatcher = function getMethodMatcher(_ref3) {
              var expectedMethod = _ref3.method;
              debug$2('Generating method matcher');

              if (!expectedMethod) {
                debug$2('  No method expectations defined - skipping');
                return;
              }

              debug$2('  Expected method:', expectedMethod);
              return function (url, _ref4) {
                var method = _ref4.method;
                debug$2('Attempting to match method');
                var actualMethod = method ? method.toLowerCase() : 'get';
                debug$2('  Expected method:', expectedMethod);
                debug$2('  Actual method:', actualMethod);
                return expectedMethod === actualMethod;
              };
            };

            var getQueryStringMatcher = function getQueryStringMatcher(_ref5) {
              var passedQuery = _ref5.query;
              debug$2('Generating query parameters matcher');

              if (!passedQuery) {
                debug$2('  No query parameters expectations defined - skipping');
                return;
              }

              var expectedQuery = querystring.parse(querystring.stringify(passedQuery));
              debug$2('  Expected query parameters:', passedQuery);
              var keys = Object.keys(expectedQuery);
              return function (url) {
                debug$2('Attempting to match query parameters');
                var query = querystring.parse(getQuery(url));
                debug$2('  Expected query parameters:', expectedQuery);
                debug$2('  Actual query parameters:', query);
                return keys.every(function (key) {
                  if (Array.isArray(query[key])) {
                    if (!Array.isArray(expectedQuery[key])) {
                      return false;
                    } else {
                      return lodash_isequal(query[key].sort(), expectedQuery[key].sort());
                    }
                  }

                  return query[key] === expectedQuery[key];
                });
              };
            };

            var getParamsMatcher = function getParamsMatcher(_ref6) {
              var expectedParams = _ref6.params,
                  matcherUrl = _ref6.url;
              debug$2('Generating path parameters matcher');

              if (!expectedParams) {
                debug$2('  No path parameters expectations defined - skipping');
                return;
              }

              if (!/express:/.test(matcherUrl)) {
                throw new Error('fetch-mock: matching on params is only possible when using an express: matcher');
              }

              debug$2('  Expected path parameters:', expectedParams);
              var expectedKeys = Object.keys(expectedParams);
              var keys = [];
              var re = pathToRegexp_1(matcherUrl.replace(/^express:/, ''), keys);
              return function (url) {
                debug$2('Attempting to match path parameters');
                var vals = re.exec(getPath(url)) || [];
                vals.shift();
                var params = keys.reduce(function (map, _ref7, i) {
                  var name = _ref7.name;
                  return vals[i] ? Object.assign(map, (0, _defineProperty2$1["default"])({}, name, vals[i])) : map;
                }, {});
                debug$2('  Expected path parameters:', expectedParams);
                debug$2('  Actual path parameters:', params);
                return expectedKeys.every(function (key) {
                  return params[key] === expectedParams[key];
                });
              };
            };

            var getBodyMatcher = function getBodyMatcher(route, fetchMock) {
              var matchPartialBody = fetchMock.getOption('matchPartialBody', route);
              var expectedBody = route.body;
              debug$2('Generating body matcher');
              return function (url, _ref8) {
                var body = _ref8.body,
                    _ref8$method = _ref8.method,
                    method = _ref8$method === void 0 ? 'get' : _ref8$method;
                debug$2('Attempting to match body');

                if (method.toLowerCase() === 'get') {
                  debug$2('  GET request - skip matching body'); // GET requests don’t send a body so the body matcher should be ignored for them

                  return true;
                }

                var sentBody;

                try {
                  debug$2('  Parsing request body as JSON');
                  sentBody = JSON.parse(body);
                } catch (err) {
                  debug$2('  Failed to parse request body as JSON', err);
                }

                debug$2('Expected body:', expectedBody);
                debug$2('Actual body:', sentBody);

                if (matchPartialBody) {
                  debug$2('matchPartialBody is true - checking for partial match only');
                }

                return sentBody && (matchPartialBody ? isSubset_1(sentBody, expectedBody) : lodash_isequal(sentBody, expectedBody));
              };
            };

            var getFullUrlMatcher = function getFullUrlMatcher(route, matcherUrl, query) {
              // if none of the special syntaxes apply, it's just a simple string match
              // but we have to be careful to normalize the url we check and the name
              // of the route to allow for e.g. http://it.at.there being indistinguishable
              // from http://it.at.there/ once we start generating Request/Url objects
              debug$2('  Matching using full url', matcherUrl);
              var expectedUrl = normalizeUrl$1(matcherUrl);
              debug$2('  Normalised url to:', matcherUrl);

              if (route.identifier === matcherUrl) {
                debug$2('  Updating route identifier to match normalized url:', matcherUrl);
                route.identifier = expectedUrl;
              }

              return function (matcherUrl) {
                debug$2('Expected url:', expectedUrl);
                debug$2('Actual url:', matcherUrl);

                if (query && expectedUrl.indexOf('?')) {
                  debug$2('Ignoring query string when matching url');
                  return matcherUrl.indexOf(expectedUrl) === 0;
                }

                return normalizeUrl$1(matcherUrl) === expectedUrl;
              };
            };

            var getFunctionMatcher = function getFunctionMatcher(_ref9) {
              var functionMatcher = _ref9.functionMatcher;
              debug$2('Detected user defined function matcher', functionMatcher);
              return function () {
                for (var _len = arguments.length, args = new Array(_len), _key = 0; _key < _len; _key++) {
                  args[_key] = arguments[_key];
                }

                debug$2('Calling function matcher with arguments', args);
                return functionMatcher.apply(void 0, args);
              };
            };

            var getUrlMatcher = function getUrlMatcher(route) {
              debug$2('Generating url matcher');
              var matcherUrl = route.url,
                  query = route.query;

              if (matcherUrl === '*') {
                debug$2('  Using universal * rule to match any url');
                return function () {
                  return true;
                };
              }

              if (matcherUrl instanceof RegExp) {
                debug$2('  Using regular expression to match url:', matcherUrl);
                return function (url) {
                  return matcherUrl.test(url);
                };
              }

              if (matcherUrl.href) {
                debug$2("  Using URL object to match url", matcherUrl);
                return getFullUrlMatcher(route, matcherUrl.href, query);
              }

              for (var shorthand in stringMatchers) {
                if (matcherUrl.indexOf(shorthand + ':') === 0) {
                  debug$2("  Using ".concat(shorthand, ": pattern to match url"), matcherUrl);
                  var urlFragment = matcherUrl.replace(new RegExp("^".concat(shorthand, ":")), '');
                  return stringMatchers[shorthand](urlFragment);
                }
              }

              return getFullUrlMatcher(route, matcherUrl, query);
            };

            var matchers = [{
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

            var _slicedToArray2$2 = interopRequireDefault(slicedToArray);

            var _classCallCheck2$2 = interopRequireDefault(classCallCheck);

            var _createClass2$1 = interopRequireDefault(createClass);

            var _typeof2$2 = interopRequireDefault(_typeof_1);



            var debug$3 = debug_1.debug,
                setDebugNamespace = debug_1.setDebugNamespace,
                getDebug$2 = debug_1.getDebug;

            var isUrlMatcher = function isUrlMatcher(matcher) {
              return matcher instanceof RegExp || typeof matcher === 'string' || (0, _typeof2$2["default"])(matcher) === 'object' && 'href' in matcher;
            };

            var isFunctionMatcher = function isFunctionMatcher(matcher) {
              return typeof matcher === 'function';
            };

            var Route = /*#__PURE__*/function () {
              function Route(args, fetchMock) {
                (0, _classCallCheck2$2["default"])(this, Route);
                this.fetchMock = fetchMock;
                var debug = getDebug$2('compileRoute()');
                debug('Compiling route');
                this.init(args);
                this.sanitize();
                this.validate();
                this.generateMatcher();
                this.limit();
                this.delayResponse();
              }

              (0, _createClass2$1["default"])(Route, [{
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
                  var _args = (0, _slicedToArray2$2["default"])(args, 3),
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
                  var debug = getDebug$2('sanitize()');
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
                  debug$3('Compiling matcher for route');
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
                  debug$3('Compiled matcher for route');
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

                  var debug = getDebug$2('limit()');
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

                  var debug = getDebug$2('delayResponse()');
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
            matchers.forEach(Route.addMatcher);
            var Route_1 = Route;

            var _regenerator$2 = interopRequireDefault(regenerator);

            var _asyncToGenerator2$2 = interopRequireDefault(asyncToGenerator);

            var _slicedToArray2$3 = interopRequireDefault(slicedToArray);

            var _toConsumableArray2$1 = interopRequireDefault(toConsumableArray);

            var setDebugPhase$2 = debug_1.setDebugPhase,
                setDebugNamespace$1 = debug_1.setDebugNamespace,
                debug$4 = debug_1.debug;

            var normalizeUrl$2 = requestUtils.normalizeUrl;



            var FetchMock$2 = {};

            var isName = function isName(nameOrMatcher) {
              return typeof nameOrMatcher === 'string' && /^[\da-zA-Z\-]+$/.test(nameOrMatcher);
            };

            var filterCallsWithMatcher = function filterCallsWithMatcher(matcher) {
              var options = arguments.length > 1 && arguments[1] !== undefined ? arguments[1] : {};
              var calls = arguments.length > 2 ? arguments[2] : undefined;

              var _Route = new Route_1([Object.assign({
                matcher: matcher,
                response: 'ok'
              }, options)], this);

              matcher = _Route.matcher;
              return calls.filter(function (_ref) {
                var url = _ref.url,
                    options = _ref.options;
                return matcher(normalizeUrl$2(url), options);
              });
            };

            var formatDebug = function formatDebug(func) {
              return function () {
                setDebugPhase$2('inspect');

                for (var _len = arguments.length, args = new Array(_len), _key = 0; _key < _len; _key++) {
                  args[_key] = arguments[_key];
                }

                var result = func.call.apply(func, [this].concat(args));
                setDebugPhase$2();
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

            FetchMock$2.filterCalls = function (nameOrMatcher, options) {
              debug$4('Filtering fetch calls');
              var calls = this._calls;
              var matcher = '*';

              if ([true, 'matched'].includes(nameOrMatcher)) {
                debug$4("Filter provided is ".concat(nameOrMatcher, ". Returning matched calls only"));
                calls = calls.filter(function (_ref2) {
                  var isUnmatched = _ref2.isUnmatched;
                  return !isUnmatched;
                });
              } else if ([false, 'unmatched'].includes(nameOrMatcher)) {
                debug$4("Filter provided is ".concat(nameOrMatcher, ". Returning unmatched calls only"));
                calls = calls.filter(function (_ref3) {
                  var isUnmatched = _ref3.isUnmatched;
                  return isUnmatched;
                });
              } else if (typeof nameOrMatcher === 'undefined') {
                debug$4("Filter provided is undefined. Returning all calls");
                calls = calls;
              } else if (isName(nameOrMatcher)) {
                debug$4("Filter provided, looks like the name of a named route. Returning only calls handled by that route");
                calls = calls.filter(function (_ref4) {
                  var identifier = _ref4.identifier;
                  return identifier === nameOrMatcher;
                });
              } else {
                matcher = nameOrMatcher === '*' ? '*' : normalizeUrl$2(nameOrMatcher);

                if (this.routes.some(function (_ref5) {
                  var identifier = _ref5.identifier;
                  return identifier === matcher;
                })) {
                  debug$4("Filter provided, ".concat(nameOrMatcher, ", identifies a route. Returning only calls handled by that route"));
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

                debug$4('Compiling filter and options to route in order to filter all calls', nameOrMatcher);
                calls = filterCallsWithMatcher.call(this, matcher, options, calls);
              }

              debug$4("Retrieved ".concat(calls.length, " calls"));
              return calls.map(callObjToArray);
            };

            FetchMock$2.calls = formatDebug(function (nameOrMatcher, options) {
              debug$4('retrieving matching calls');
              return this.filterCalls(nameOrMatcher, options);
            });
            FetchMock$2.lastCall = formatDebug(function (nameOrMatcher, options) {
              debug$4('retrieving last matching call');
              return (0, _toConsumableArray2$1["default"])(this.filterCalls(nameOrMatcher, options)).pop();
            });
            FetchMock$2.lastUrl = formatDebug(function (nameOrMatcher, options) {
              debug$4('retrieving url of last matching call');
              return (this.lastCall(nameOrMatcher, options) || [])[0];
            });
            FetchMock$2.lastOptions = formatDebug(function (nameOrMatcher, options) {
              debug$4('retrieving options of last matching call');
              return (this.lastCall(nameOrMatcher, options) || [])[1];
            });
            FetchMock$2.lastResponse = formatDebug(function (nameOrMatcher, options) {
              debug$4('retrieving respose of last matching call');
              console.warn("When doing all the following:\n- using node-fetch\n- responding with a real network response (using spy() or fallbackToNetwork)\n- using `fetchMock.LastResponse()`\n- awaiting the body content\n... the response will hang unless your source code also awaits the response body.\nThis is an unavoidable consequence of the nodejs implementation of streams.\n");
              var response = (this.lastCall(nameOrMatcher, options) || []).response;

              try {
                var clonedResponse = response.clone();
                return clonedResponse;
              } catch (err) {
                Object.entries(response._fmResults).forEach(function (_ref6) {
                  var _ref7 = (0, _slicedToArray2$3["default"])(_ref6, 2),
                      name = _ref7[0],
                      result = _ref7[1];

                  response[name] = function () {
                    return result;
                  };
                });
                return response;
              }
            });
            FetchMock$2.called = formatDebug(function (nameOrMatcher, options) {
              debug$4('checking if matching call was made');
              return Boolean(this.filterCalls(nameOrMatcher, options).length);
            });
            FetchMock$2.flush = formatDebug( /*#__PURE__*/function () {
              var _ref8 = (0, _asyncToGenerator2$2["default"])( /*#__PURE__*/_regenerator$2["default"].mark(function _callee(waitForResponseMethods) {
                var queuedPromises;
                return _regenerator$2["default"].wrap(function _callee$(_context) {
                  while (1) {
                    switch (_context.prev = _context.next) {
                      case 0:
                        setDebugNamespace$1('flush');
                        debug$4("flushing all fetch calls. ".concat(waitForResponseMethods ? '' : 'Not ', "waiting for response bodies to complete download"));
                        queuedPromises = this._holdingPromises;
                        this._holdingPromises = [];
                        debug$4("".concat(queuedPromises.length, " fetch calls to be awaited"));
                        _context.next = 7;
                        return Promise.all(queuedPromises);

                      case 7:
                        debug$4("All fetch calls have completed");

                        if (!(waitForResponseMethods && this._holdingPromises.length)) {
                          _context.next = 13;
                          break;
                        }

                        debug$4("Awaiting all fetch bodies to download");
                        _context.next = 12;
                        return this.flush(waitForResponseMethods);

                      case 12:
                        debug$4("All fetch bodies have completed downloading");

                      case 13:
                        setDebugNamespace$1();

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
            FetchMock$2.done = formatDebug(function (nameOrMatcher) {
              var _this = this;

              setDebugPhase$2('inspect');
              setDebugNamespace$1('done');
              debug$4('Checking to see if expected calls have been made');
              var routesToCheck;

              if (nameOrMatcher && typeof nameOrMatcher !== 'boolean') {
                debug$4('Checking to see if expected calls have been made for single route:', nameOrMatcher);
                routesToCheck = [{
                  identifier: nameOrMatcher
                }];
              } else {
                debug$4('Checking to see if expected calls have been made for all routes');
                routesToCheck = this.routes;
              } // Can't use array.every because would exit after first failure, which would
              // break the logging


              var result = routesToCheck.map(function (_ref9) {
                var identifier = _ref9.identifier;

                if (!_this.called(identifier)) {
                  debug$4('No calls made for route:', identifier);
                  console.warn("Warning: ".concat(identifier, " not called")); // eslint-disable-line

                  return false;
                }

                var expectedTimes = (_this.routes.find(function (r) {
                  return r.identifier === identifier;
                }) || {}).repeat;

                if (!expectedTimes) {
                  debug$4('Route has been called at least once, and no expectation of more set:', identifier);
                  return true;
                }

                var actualTimes = _this.filterCalls(identifier).length;

                debug$4("Route called ".concat(actualTimes, " times:"), identifier);

                if (expectedTimes > actualTimes) {
                  debug$4("Route called ".concat(actualTimes, " times, but expected ").concat(expectedTimes, ":"), identifier);
                  console.warn("Warning: ".concat(identifier, " only called ").concat(actualTimes, " times, but ").concat(expectedTimes, " expected")); // eslint-disable-line

                  return false;
                } else {
                  return true;
                }
              }).every(function (isDone) {
                return isDone;
              });
              setDebugNamespace$1();
              setDebugPhase$2();
              return result;
            });
            var inspecting = FetchMock$2;

            var debug$5 = debug_1.debug;









            var FetchMock$3 = Object.assign({}, fetchHandler, setUpAndTearDown, inspecting);

            FetchMock$3.addMatcher = function (matcher) {
              Route_1.addMatcher(matcher);
            };

            FetchMock$3.config = {
              fallbackToNetwork: false,
              includeContentLength: true,
              sendAsJson: true,
              warnOnFallback: true,
              overwriteRoutes: undefined
            };

            FetchMock$3.createInstance = function () {
              var _this = this;

              debug$5('Creating fetch-mock instance');
              var instance = Object.create(FetchMock$3);
              instance._uncompiledRoutes = (this._uncompiledRoutes || []).slice();
              instance.routes = instance._uncompiledRoutes.map(function (config) {
                return _this.compileRoute(config);
              });
              instance.fallbackResponse = this.fallbackResponse || undefined;
              instance.config = Object.assign({}, this.config || FetchMock$3.config);
              instance._calls = [];
              instance._holdingPromises = [];
              instance.bindMethods();
              return instance;
            };

            FetchMock$3.compileRoute = function (config) {
              return new Route_1(config, this);
            };

            FetchMock$3.bindMethods = function () {
              this.fetchHandler = FetchMock$3.fetchHandler.bind(this);
              this.reset = this.restore = FetchMock$3.reset.bind(this);
              this.resetHistory = FetchMock$3.resetHistory.bind(this);
              this.resetBehavior = FetchMock$3.resetBehavior.bind(this);
            };

            FetchMock$3.sandbox = function () {
              debug$5('Creating sandboxed fetch-mock instance'); // this construct allows us to create a fetch-mock instance which is also
              // a callable function, while circumventing circularity when defining the
              // object that this function should be bound to

              var fetchMockProxy = function fetchMockProxy(url, options) {
                return sandbox.fetchHandler(url, options);
              };

              var sandbox = Object.assign(fetchMockProxy, // Ensures that the entire returned object is a callable function
              FetchMock$3, // prototype methods
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

            FetchMock$3.getOption = function (name) {
              var route = arguments.length > 1 && arguments[1] !== undefined ? arguments[1] : {};
              return name in route ? route[name] : this.config[name];
            };

            var lib = FetchMock$3;

            var statusTextMap = {
              100: 'Continue',
              101: 'Switching Protocols',
              102: 'Processing',
              200: 'OK',
              201: 'Created',
              202: 'Accepted',
              203: 'Non-Authoritative Information',
              204: 'No Content',
              205: 'Reset Content',
              206: 'Partial Content',
              207: 'Multi-Status',
              208: 'Already Reported',
              226: 'IM Used',
              300: 'Multiple Choices',
              301: 'Moved Permanently',
              302: 'Found',
              303: 'See Other',
              304: 'Not Modified',
              305: 'Use Proxy',
              307: 'Temporary Redirect',
              308: 'Permanent Redirect',
              400: 'Bad Request',
              401: 'Unauthorized',
              402: 'Payment Required',
              403: 'Forbidden',
              404: 'Not Found',
              405: 'Method Not Allowed',
              406: 'Not Acceptable',
              407: 'Proxy Authentication Required',
              408: 'Request Timeout',
              409: 'Conflict',
              410: 'Gone',
              411: 'Length Required',
              412: 'Precondition Failed',
              413: 'Payload Too Large',
              414: 'URI Too Long',
              415: 'Unsupported Media Type',
              416: 'Range Not Satisfiable',
              417: 'Expectation Failed',
              418: "I'm a teapot",
              421: 'Misdirected Request',
              422: 'Unprocessable Entity',
              423: 'Locked',
              424: 'Failed Dependency',
              425: 'Unordered Collection',
              426: 'Upgrade Required',
              428: 'Precondition Required',
              429: 'Too Many Requests',
              431: 'Request Header Fields Too Large',
              451: 'Unavailable For Legal Reasons',
              500: 'Internal Server Error',
              501: 'Not Implemented',
              502: 'Bad Gateway',
              503: 'Service Unavailable',
              504: 'Gateway Timeout',
              505: 'HTTP Version Not Supported',
              506: 'Variant Also Negotiates',
              507: 'Insufficient Storage',
              508: 'Loop Detected',
              509: 'Bandwidth Limit Exceeded',
              510: 'Not Extended',
              511: 'Network Authentication Required'
            };
            var statusText = statusTextMap;

            var theGlobal = typeof window !== 'undefined' ? window : self;

            var setUrlImplementation = requestUtils.setUrlImplementation;

            setUrlImplementation(theGlobal.URL);
            lib.global = theGlobal;
            lib.statusTextMap = statusText;
            lib.config = Object.assign(lib.config, {
              Promise: theGlobal.Promise,
              Request: theGlobal.Request,
              Response: theGlobal.Response,
              Headers: theGlobal.Headers
            });
            var client = lib.createInstance();

            return client;

})));
