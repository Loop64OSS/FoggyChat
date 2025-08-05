import { isEqual } from "@zag-js/utils";
const access = (value) => {
    if (typeof value === "function")
        return value();
    return value;
};
export const track = (deps, effect) => {
    let prevDeps = [];
    let isFirstRun = true;
    $effect(() => {
        if (isFirstRun) {
            prevDeps = deps.map((d) => access(d));
            isFirstRun = false;
            return;
        }
        let changed = false;
        for (let i = 0; i < deps.length; i++) {
            if (!isEqual(prevDeps[i], access(deps[i]))) {
                changed = true;
                break;
            }
        }
        if (changed) {
            prevDeps = deps.map((d) => access(d));
            effect();
        }
    });
};
