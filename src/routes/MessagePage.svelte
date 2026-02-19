<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
    import { onMount, onDestroy, tick } from "svelte";
    import {
        LogOut,
        SendHorizontal,
        Server,
        Users,
        Settings,
        X,
        Plus,
        FileKey2,
        MessageSquareOff,
        Info,
    } from "@lucide/svelte";
    import { serverAddress } from "../lib/store.js";
    import { toaster } from "../lib/toaster-svelte";
    import { sendNotification } from "@tauri-apps/plugin-notification";

    const appWebview = getCurrentWebviewWindow();
    let message = "";
    let recipient = "";
    let sidebarOpen = false;
    let isMobile = false;

    interface Message {
        text: string;
        fromSelf: boolean;
        senderId: string;
    }

    let chatHistories: Record<string, Message[]> = {};

    $: currentMessages = chatHistories[recipient] || [];
    let messagesContainer: HTMLDivElement;
    let AddedRecipientUserName = "";
    let AddedRecipientDisplayName = "";
    let AddedRecipientPublicKey = "";
    let unlistenFctp: (() => void) | undefined;
    let unlistenCheckRecipientKey: (() => void) | undefined;
    let unlistenServerNotResponding: (() => void) | undefined;
    let unlistenBlur: (() => void) | undefined;
    let unlistenFocus: (() => void) | undefined;
    let isBlurred = false;

    interface Recipient {
        id: string;
        name: string;
        pk: string;
    }
    let addedUsers: Recipient[] = [];
    function addRecipient() {
        const exists = addedUsers.some(
            (user) => user.id === AddedRecipientUserName,
        );

        if (!exists) {
            addedUsers = [
                ...addedUsers,
                {
                    id: AddedRecipientUserName,
                    name: AddedRecipientDisplayName,
                    pk: AddedRecipientPublicKey,
                },
            ];
            selectRecipient(AddedRecipientUserName);
        }

        AddedRecipientUserName = "";
        AddedRecipientDisplayName = "";
        AddedRecipientPublicKey = "";
    }
    function checkMobile() {
        isMobile = window.innerWidth < 640;
        if (!isMobile) {
            sidebarOpen = true;
        }
    }

    onMount(async () => {
        checkMobile();
        window.addEventListener("resize", checkMobile);
        AddedRecipientDisplayName = "Server (" + $serverAddress + ")";
        AddedRecipientUserName = "server";
        addRecipient();

        // Register Tauri webview listeners and keep unlisten handles
        try {
            // Track webview focus/blur to avoid sending notifications when focused
            unlistenBlur = await appWebview.listen("tauri://blur", () => {
                isBlurred = true;
            });
            unlistenFocus = await appWebview.listen("tauri://focus", () => {
                isBlurred = false;
            });

            unlistenFctp = await appWebview.listen<any>(
                "fctp-message",
                async (event) => {
                    const { sender, content } = event.payload;

                    addMessage(content, sender);
                    // Send a native notification only when the webview is blurred

                    if (!sender.endsWith("*")) {
                        if (isBlurred || sender != recipient) {
                            sendNotification({
                                title: "FoggyChat",
                                body: sender + ": " + content,
                            });
                        }
                        addedUsers.forEach((user) => {
                            if (sender != user) {
                                AddedRecipientDisplayName = sender;
                                AddedRecipientUserName = sender;
                                addRecipient();
                                AddedRecipientDisplayName = "";
                                AddedRecipientDisplayName = "";
                            }
                        });
                    }
                },
            );

            unlistenCheckRecipientKey = await appWebview.listen(
                "ui_command_check_recipient_key",
                (event) => {
                    const payload: any = event.payload;
                    if (payload && payload.status === "ok") {
                        addMessage(`Key (base64): ${payload.key}`, "server");
                    } else if (payload && payload.status === "error") {
                        addMessage(`Error: ${payload.message}`, "server");
                    } else {
                        addMessage(JSON.stringify(payload), "server");
                    }
                    console.log(payload);
                },
            );

            unlistenServerNotResponding = await appWebview.listen(
                "ui_info_server_not_responding",
                (event) => {
                    const payload: any = event.payload;
                    console.log(payload);
                    toaster.info({
                        title: "Server not responding",
                        description: payload,
                    });
                },
            );
        } catch (e) {
            console.warn("Failed to register webview listeners", e);
        }
    });

    function sendMessage() {
        if (recipient === "server") {
            message = "/" + message;
        }
        invoke("ui_command_send_fctp_message", {
            message: message,
            recipient: recipient,
        });
        message = "";
    }

    function sendStatus(status: string) {
        invoke("ui_command_status", { input: status });
    }
    function checkRecipientKey(userId: string) {
        invoke("ui_command_check_recipient_key", { recipient: userId });
    }

    function selectRecipient(userId: string) {
        recipient = userId;
        invoke("ui_command_select_recipient", { recipient: userId });
        if (isMobile) {
            sidebarOpen = false;
        }
    }
    function removeRecipient(id: string): void {
        // Remove the user from the addedUsers list
        invoke("ui_command_remove_recipient", { recipient: id });
        addedUsers = addedUsers.filter((u) => u.id !== id);
        chatHistories = {
            ...chatHistories,
            [id]: [],
        }; // If the removed user was the currently selected recipient, clear selection
        if (recipient === id) {
            recipient = "";
            // notify backend about deselection
            selectRecipient(recipient);
        }
    }
    const scrollToBottom = async (obj: HTMLDivElement) => {
        obj.scroll({ top: obj.scrollHeight, behavior: "smooth" });
    };
    async function addMessage(text: string, senderId: string) {
        let fromSelf = false;
        if (senderId.endsWith("*")) {
            fromSelf = true;
            senderId = senderId.slice(0, -1);
        }

        const messageObj: Message = {
            text,
            fromSelf,
            senderId,
        };

        chatHistories = {
            ...chatHistories,
            [senderId]: [...(chatHistories[senderId] || []), messageObj],
        };

        await tick();

        if (recipient === senderId && messagesContainer) {
            scrollToBottom(messagesContainer);
        }
        if (senderId == "server") {
            toaster.info({
                title: "Server",
                description: text,
                closable: true,
                type: "info",
            });
        }
    }

    async function copyMessage(text: string) {
        try {
            await navigator.clipboard.writeText(text);
            toaster.info({
                title: "Copied",
                description: "The message has been added to the clipboard",
                closable: true,
                type: "success",
            });
        } catch (e) {
            toaster.info({
                title: "Error",
                description: "Couldn't copy the message",
                closable: true,
                type: "error",
            });
        }
    }

    onDestroy(() => {
        window.removeEventListener("resize", checkMobile);
        if (unlistenFctp) {
            try {
                unlistenFctp();
            } catch (e) {
                console.warn("Failed to unlisten fctp-message", e);
            }
        }
        if (unlistenCheckRecipientKey) {
            try {
                unlistenCheckRecipientKey();
            } catch (e) {
                console.warn(
                    "Failed to unlisten ui_command_check_recipient_key",
                    e,
                );
            }
        }
        if (unlistenServerNotResponding) {
            try {
                unlistenServerNotResponding();
            } catch (e) {
                console.warn(
                    "Failed to unlisten ui_info_server_not_responding",
                    e,
                );
            }
        }
        if (unlistenBlur) {
            try {
                unlistenBlur();
            } catch (e) {
                console.warn("Failed to unlisten tauri://blur", e);
            }
        }
        if (unlistenFocus) {
            try {
                unlistenFocus();
            } catch (e) {
                console.warn("Failed to unlisten tauri://focus", e);
            }
        }
    });

    function disconnectFromServer() {
        sendStatus("USER::DISCONNECT");
    }

    function toggleSidebar() {
        sidebarOpen = !sidebarOpen;
    }
</script>

<main class="flex h-full relative">
    <!-- Mobile Overlay -->
    {#if isMobile && sidebarOpen}
        <button
            class="fixed inset-0 bg-surface-50-950/50 md:hidden z-50 cursor-default"
            on:click={() => (sidebarOpen = false)}
            aria-label="Close sidebar"
        ></button>
    {/if}

    <!-- Sidebar -->
    <div
        class={`fixed sm:relative z-200
        max-sm:left-0 max-sm:top-0 max-sm:h-full
        transition-all duration-300 
        ${sidebarOpen ? "max-sm:translate-x-0 w-80" : isMobile ? "w-80 -translate-x-full" : "w-0"} 
        overflow-hidden 
        border-r
        rounded-r-container
        border-r-surface-200-800 
        body-background-color-dark
    `}
    >
        <div class="flex flex-col h-full">
            <!-- Sidebar Header -->
            <div class="p-4 border-b border-b-surface-200-800">
                <div class="flex items-center justify-between mb-4">
                    <div class="flex items-center gap-3">
                        <div class="preset-filled-primary-500 p-2 rounded-base">
                            <Users size={20} />
                        </div>
                        <h2 class="text-lg font-semibold">Users</h2>
                    </div>
                    {#if isMobile}
                        <button
                            on:click={() => (sidebarOpen = false)}
                            class="preset-outlined-surface-200-800 p-2 rounded-base hover:scale-105 transition-transform"
                        >
                            <X size={20} />
                        </button>
                    {/if}
                </div>

                <!-- Search Bar -->
                <div class="relative">
                    <form
                        on:submit|preventDefault={addRecipient}
                        class="space-y-2"
                    >
                        <div class="flex flex-row space-x-2">
                            <input
                                bind:value={AddedRecipientUserName}
                                type="text"
                                placeholder="Username..."
                                class="input w-full focus:ring-2 focus:ring-primary-500 rounded-base text-sm"
                                required
                            />
                            <input
                                bind:value={AddedRecipientDisplayName}
                                type="text"
                                placeholder="Display name..."
                                class="input w-full focus:ring-2 focus:ring-primary-500 rounded-base text-sm"
                                required
                            />
                        </div>
                        <button
                            type="submit"
                            class="btn px-3 preset-filled-surface-200-800 w-full rounded-base hover:scale-105 active:scale-95 transition-transform disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100"
                            ><Plus size={20} /> Add recipient</button
                        >
                    </form>
                </div>
            </div>

            <!-- Users List -->
            <div class="flex-1 overflow-y-auto p-4">
                <div class="space-y-1">
                    {#each addedUsers as user}
                        <div
                            class={`w-full rounded-base text-left hover:preset-filled-primary-300-700 transition-colors flex items-center justify-between ${recipient === user.id ? "preset-filled-primary-500 outline-2 " : ""}`}
                        >
                            <button
                                on:click={() => selectRecipient(user.id)}
                                class="flex p-3 items-center gap-3 flex-1 text-left min-w-0"
                            >
                                <div class="relative shrink-0">
                                    {#if user.id == "server"}
                                        <div
                                            class="w-10 h-10 flex items-center justify-center font-medium"
                                        >
                                            <Server size="24" />
                                        </div>
                                    {:else}
                                        <div
                                            class="w-10 h-10 rounded-base flex items-center justify-center font-medium preset-filled-primary-100-900"
                                        >
                                            <b>{user.name.charAt(0)}</b>
                                        </div>
                                    {/if}
                                </div>
                                <div class="flex-1 min-w-0 overflow-hidden">
                                    <p
                                        class={`font-medium break-all ${
                                            user.id === "server"
                                                ? "font-mono"
                                                : ""
                                        }`}
                                    >
                                        {user.name}
                                    </p>
                                </div>
                            </button>

                            <!-- If the user is not server, show the user controls -->
                            {#if user.id != "server"}
                                <div class="flex items-center shrink-0">
                                    <button
                                        class="p-1 rounded-base hover:preset-filled-primary-100-900"
                                        title="Verify authenticity of recipient key"
                                        on:click={() =>
                                            checkRecipientKey(user.id)}
                                    >
                                        <FileKey2 size={24} />
                                    </button>
                                    <button
                                        class="p-1 mr-2 rounded-base hover:preset-filled-primary-100-900"
                                        title="Remove a recipient from the list"
                                        on:click={() =>
                                            removeRecipient(user.id)}
                                    >
                                        <X size={24} />
                                    </button>
                                </div>
                            {/if}
                        </div>
                    {/each}
                </div>
            </div>

            <!-- Sidebar Footer -->
            <div class="p-4 border-t border-t-surface-200-800">
                <button
                    class="w-full flex items-center gap-2 p-2 hover:preset-filled-surface-100-900 rounded-base transition-colors opacity-70 hover:opacity-100"
                >
                    <Settings size={16} />
                    <span class="text-sm disabled">Settings</span>
                </button>
            </div>
        </div>
    </div>

    <!-- Main Chat Area -->
    <div class="flex-1 flex flex-col min-w-0">
        <!-- Header -->
        <header
            class="gap-2 grid-cols-[auto_1fr_auto] justify-between mt-2 mx-4 card rounded-container items-stretch"
        >
            <div
                class="grid grid-cols-[auto_1fr_auto] gap-2 items-center preset-outlined-surface-200-800 card p-2"
            >
                <div class="flex items-center gap-2">
                    <button
                        on:click={toggleSidebar}
                        class="btn btn-sm preset-outlined-surface-200-800 p-1 hover:scale-105 active:scale-95 transition-transform"
                        title="Toggle side panel"
                    >
                        <Users size={16} />
                    </button>

                    <button
                        class="btn btn-sm preset-outlined-surface-200-800 p-1 hover:scale-105 active:scale-95 transition-transform"
                        title="You are connected to: {$serverAddress}"
                    >
                        <Info size={16} />
                    </button>
                </div>
                <div></div>
                <button
                    class="btn btn-sm preset-filled-error-200-800 hover:scale-105 active:scale-95 transition-transform"
                    on:click={disconnectFromServer}
                >
                    <span class="hidden sm:inline">Leave</span>
                    <LogOut size={16} />
                </button>
            </div>
        </header>

        <!-- Messages Container -->
        <div
            bind:this={messagesContainer}
            id="message-container"
            class="flex-1 overflow-y-scroll m-4 space-y-2 h-full card"
        >
            {#if currentMessages.length === 0}
                <div
                    class="flex flex-col items-center justify-center h-full text-center p-4"
                >
                    <MessageSquareOff size={48} class="opacity-50 mb-4" />
                    <h3 class="text-lg font-medium opacity-70 mb-2">
                        No messages yet
                    </h3>
                    <p class="text-sm opacity-50">
                        {recipient
                            ? `Start chatting with ${recipient}!`
                            : "Select a user and start chatting!"}
                    </p>
                </div>
            {:else}
                {#each currentMessages as msg, index}
                    <div class="flex wrap-break-word">
                        <div
                            role="button"
                            tabindex="0"
                            on:click={() => copyMessage(msg.text)}
                            on:keydown={(e) =>
                                (e.key === "Enter" || e.key === " ") &&
                                copyMessage(msg.text)}
                            class={`max-w-[70%] p-3 wrap-break-word rounded-base cursor-pointer select-text ${msg.fromSelf ? "ml-auto bg-primary-500 text-white" : "mr-auto preset-filled-surface-100-900"}`}
                            title="Click to copy a message"
                        >
                            <p class="text-sm leading-relaxed">{msg.text}</p>
                        </div>
                    </div>
                {/each}
            {/if}
        </div>

        <!-- Message Input -->
        <div class="mx-4 mb-4">
            <form
                class="w-full card preset-outlined-surface-200-800 p-3 rounded-container"
                on:submit|preventDefault={sendMessage}
            >
                <div class="grid grid-cols-[1fr_auto] gap-2">
                    <input
                        class="input rounded-base focus:ring-2 focus:ring-primary-500"
                        type="text"
                        placeholder={recipient
                            ? `Message ${recipient}...`
                            : "Select a recipient first..."}
                        bind:value={message}
                        disabled={!recipient}
                        required
                    />

                    <button
                        class="btn px-3 preset-filled-surface-200-800 rounded-base hover:scale-105 active:scale-95 transition-transform disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100"
                        type="submit"
                        disabled={!recipient || !message.trim()}
                    >
                        <SendHorizontal size={20} />
                    </button>
                </div>
            </form>
        </div>
    </div>
</main>
