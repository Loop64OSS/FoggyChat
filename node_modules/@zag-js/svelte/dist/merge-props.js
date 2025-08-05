import { mergeProps as zagMergeProps } from "@zag-js/core";
import { toStyleString } from "./normalize-props";
const CSS_REGEX = /((?:--)?(?:\w+-?)+)\s*:\s*([^;]*)/g;
const serialize = (style) => {
    const res = {};
    let match;
    while ((match = CSS_REGEX.exec(style))) {
        res[match[1]] = match[2];
    }
    return res;
};
export function mergeProps(...args) {
    const merged = zagMergeProps(...args);
    if ("style" in merged) {
        if (typeof merged.style === "string") {
            merged.style = serialize(merged.style);
        }
        merged.style = toStyleString(merged.style);
    }
    return merged;
}
