<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
    import { onMount, tick } from "svelte";
    import { navigate } from "svelte-routing";
    import { X } from "@lucide/svelte";
    import { exit } from "@tauri-apps/plugin-process";

    const appWebview = getCurrentWebviewWindow();
    var message = "";
    let messages: string[] = [];
    let messagesContainer: HTMLDivElement;

    onMount(() => scrollToBottom(messagesContainer));

    const scrollToBottom = async (node: HTMLDivElement) => {
        node.scroll({ top: node.scrollHeight, behavior: "smooth" });
    };
    function sendMessage() {
        invoke("send_message", { line: message });
        message = "";
    }
    async function addMessage(text: string) {
        messages = [...messages, text];
        await tick();
        scrollToBottom(messagesContainer);
    }
    appWebview.listen<string>("pass-message", (event) => {
        addMessage(event.payload);
        console.log(event.payload);
    });
</script>

<main class="h-dvh flex flex-col">
    <div
        bind:this={messagesContainer}
        id="message-container"
        class="flex-1 overflow-y-scroll p-2 space-y-2 h-full"
    >
        {#each messages as msg}
            <p class="card preset-filled-surface-100-900 p-4">{msg}</p>
        {/each}
    </div>

    <form
        class="sticky w-full card body-background-color dark:body-background-color-dark"
        on:submit|preventDefault={sendMessage}
    >
        <hr class="hr" />

        <div class="input-group grid-cols-[1fr_auto] m-4">
            <input
                class="ig-input"
                id="msginput"
                type="text"
                placeholder="Input"
                bind:value={message}
            />
            <button class="ig-btn" type="submit">Send</button>
        </div>
    </form>
</main>
