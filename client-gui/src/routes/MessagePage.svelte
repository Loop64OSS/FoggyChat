<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { listen } from "@tauri-apps/api/event";
    import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
    import { onDestroy, onMount, tick } from "svelte";
    import { navigate } from "svelte-routing";
    import { toaster } from "../lib/toaster-svelte";
    import {
        LogOut,
        MessageCircleHeart,
        SendHorizontal,
        Server,
        WatchIcon,
    } from "@lucide/svelte";
    import { serverAddress } from "../lib/store.js";
    const appWebview = getCurrentWebviewWindow();
    var message = "";
    var recipient = "";

    let messages: string[] = [];
    let messagesContainer: HTMLDivElement;

    function sendMessage() {
        invoke("ui_command_send_fctp_message", {
            message: message,
            recipient: recipient,
        });
        message = "";
    }
    function sendStatus(status: string) {
        invoke("ui_command_status", { input: status });
    }

    const scrollToBottom = async (obj: HTMLDivElement) => {
        obj.scroll({ top: obj.scrollHeight, behavior: "smooth" });
    };
    async function addMessage(text: string) {
        let wasAtBottom =
            messagesContainer.scrollHeight -
                messagesContainer.scrollTop -
                messagesContainer.clientHeight <
            80;
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

    function disconnectFromServer() {
        sendStatus("USER::DISCONNECT");
    }
</script>

<main class="flex flex-col">
    <header
        class="grid sm:grid-cols-[auto_1fr_auto] gap-2 sm:mr-0 grid-cols-1 justify-between mt-2 mx-2 card items-stretch"
    >
        <div
            class="grid grid-cols-[auto_auto] gap-2 items-center preset-outlined-surface-200-800 card p-2"
        >
            <div class="flex items-center gap-2">
                <Server size={20} />
                <h1>{$serverAddress}</h1>
            </div>
            <button
                class="btn preset-filled-error-200-800 hover:scale-105 active:scale-95 transition-transform"
                on:click={disconnectFromServer}
            >
                Leave
                <LogOut size={16} />
            </button>
        </div>
        <input
            class="input focus:ring-2 focus:ring-primary-500 rounded-xl"
            id="msginput"
            type="text"
            placeholder="Enter recipient"
            bind:value={recipient}
        />
    </header>
    <div
        bind:this={messagesContainer}
        id="message-container"
        class="flex-1 overflow-y-scroll m-4 space-y-2 h-full card"
    >
        {#each messages as msg}
            <p
                class="card preset-filled-surface-100-900 p-3 text-ellipsis break-words"
            >
                {msg}
            </p>
        {/each}
    </div>
    <hr class="hr" />

    <form
        class="sticky w-full card body-background-color dark:body-background-color-dark p-3 rounded-xl shadow-lg"
        on:submit|preventDefault={sendMessage}
    >
        <div class="grid grid-cols-[1fr_auto] gap-2">
            <input
                class="input rounded-lg focus:ring-2 focus:ring-primary-500"
                id="msginput"
                type="text"
                placeholder="Write a message..."
                bind:value={message}
                required
            />

            <button
                class="btn px-3 preset-filled-surface-200-800 rounded-lg hover:scale-105 active:scale-95 transition-transform"
                type="submit"
            >
                <SendHorizontal size={20} />
            </button>
        </div>
    </form>
</main>
