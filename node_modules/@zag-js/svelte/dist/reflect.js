const isFunction = (value) => typeof value === "function";
export function reflect(obj) {
    return new Proxy(obj(), {
        get(_, prop) {
            const target = obj();
            let value = Reflect.get(target, prop);
            // @ts-ignore
            return isFunction(value) ? value.bind(target) : value;
        },
    });
}
