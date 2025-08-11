<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
    import { onMount, tick } from "svelte";
    import { navigate } from "svelte-routing";
    import { LogOut, MessageCircleHeart, Server } from "@lucide/svelte";
    import { serverAddress } from "../lib/store.js";
    const appWebview = getCurrentWebviewWindow();
    var message = "";
    let messages: string[] = [];
    let messagesContainer: HTMLDivElement;

    onMount(() => scrollToBottom(messagesContainer));
    window.onbeforeunload = function () {
        return;
    };
    const scrollToBottom = async (obj: HTMLDivElement) => {
        obj.scroll({ top: obj.scrollHeight, behavior: "smooth" });
    };
    async function addMessage(text: string) {
        let wasAtBottom =
            messagesContainer.scrollHeight -
                messagesContainer.scrollTop -
                messagesContainer.clientHeight <
            1;
        messages = [...messages, text];
        await tick();
        if (wasAtBottom) {
            scrollToBottom(messagesContainer);
        }
    }
    appWebview.listen<string>("fctp-message", (event) => {
        addMessage(event.payload);
        console.log(event.payload);
    });
    function sendMessage() {
        invoke("ui_command_send_fctp_message", { input: message });
        message = "";
    }
    function sendStatus(status: string) {
        invoke("ui_command_status", { input: status });
    }
    function disconnectFromServer() {
        navigate("/");
        sendStatus("USER::DISCONNECT");
    }
</script>

<main class="flex flex-col">
    <header class="flex justify-between items-center px-2 pt-2 card">
        <div class="flex items-center">
            <Server size={20} class="mr-2" />
            <h1>{$serverAddress}</h1>
        </div>
        <button
            class="btn preset-filled-error-200-800 p-2"
            on:click={disconnectFromServer}
        >
            <LogOut size={16} />
        </button>
    </header>
    <div
        bind:this={messagesContainer}
        id="message-container"
        class="flex-1 overflow-y-scroll m-4 mt-2 space-y-2 h-full"
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
