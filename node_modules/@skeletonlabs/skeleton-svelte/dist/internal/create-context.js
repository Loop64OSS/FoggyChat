import { getContext, setContext } from 'svelte';
/**
 * Create a context with a default value
 * @param defaultValue The default value that will be returned if the context is not set
 * @returns [set, get, key] The setter, getter and key for the context, the getter returns the default value if the context is not set
 */
export function createContext(defaultValue) {
    var key = Symbol();
    var set = function (value) { return setContext(key, value); };
    var get = function () { var _a; return (_a = getContext(key)) !== null && _a !== void 0 ? _a : defaultValue; };
    return [set, get, key];
}
