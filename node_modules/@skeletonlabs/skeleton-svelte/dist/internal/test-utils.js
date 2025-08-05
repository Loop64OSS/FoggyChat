import { createRawSnippet } from 'svelte';
/** Use `mockSnippet(value)` to create `<span>{value}</span>` snippet content. */
export function mockSnippet(key) {
    return createRawSnippet(function () {
        return {
            render: function () { return "<span>".concat(key, "</span>"); }
        };
    });
}
