export function useRefs(refs) {
    const ref = $state({ current: refs });
    return {
        get(key) {
            return ref.current[key];
        },
        set(key, value) {
            ref.current[key] = value;
        },
    };
}
