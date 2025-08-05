export function portal(node, props = {}) {
    function update(props = {}) {
        const { container, disabled, getRootNode } = props;
        if (disabled)
            return;
        const doc = getRootNode?.().ownerDocument ?? document;
        const mountNode = container ?? doc.body;
        mountNode.appendChild(node);
    }
    update(props);
    return {
        destroy: () => node.remove(),
        update,
    };
}
