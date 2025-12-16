<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
    import { onMount, tick } from "svelte";
    import {
        LogOut,
        SendHorizontal,
        Server,
        Users,
        Settings,
        Menu,
        X,
        Plus,
        FileKey2,
        MessageSquareOff,
    } from "@lucide/svelte";
    import { serverAddress } from "../lib/store.js";

    const appWebview = getCurrentWebviewWindow();
    let message = "";
    let recipient = "";
    let sidebarOpen = false;
    let isMobile = false;

    let messages: string[] = [];
    let messagesContainer: HTMLDivElement;
    let AddedRecipientUserName = "";
    let AddedRecipientDisplayName = "";
    let AddedRecipientPublicKey = "";

    interface Recipient {
        id: string;
        name: string;
        pk: string;
    }
    let addedUsers: Recipient[] = [];
    function addRecipient() {
        addedUsers = [
            ...addedUsers,
            {
                id: AddedRecipientUserName,
                name: AddedRecipientDisplayName,
                pk: AddedRecipientPublicKey,
            },
        ];
        selectRecipient(AddedRecipientUserName);

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

    onMount(() => {
        checkMobile();
        window.addEventListener("resize", checkMobile);
        AddedRecipientDisplayName = "Server (" + $serverAddress + ")";
        AddedRecipientUserName = "server";
        addRecipient();
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

        // If the removed user was the currently selected recipient, clear selection
        if (recipient === id) {
            recipient = "";
            // notify backend about deselection
            selectRecipient(recipient);
        }
    }
    const scrollToBottom = async (obj: HTMLDivElement) => {
        obj.scroll({ top: obj.scrollHeight, behavior: "smooth" });
    };
    appWebview.listen<string>("fctp-message", (event) => {
        addMessage(event.payload);
        console.log(event.payload);
    });
    appWebview.listen("ui_command_check_recipient_key", (event) => {
        const payload: any = event.payload;
        if (payload && payload.status === "ok") {
            addMessage(`Key (base64): ${payload.key}`);
        } else if (payload && payload.status === "error") {
            addMessage(`Error: ${payload.message}`);
        } else {
            addMessage(JSON.stringify(payload));
        }
        console.log(payload);
    });
    async function addMessage(text: string) {
        let wasAtBottom = false;
        if (messagesContainer) {
            wasAtBottom =
                messagesContainer.scrollHeight -
                    messagesContainer.scrollTop -
                    messagesContainer.clientHeight <
                80;
        }
        messages = [...messages, text];
        await tick();
        if (wasAtBottom && messagesContainer) {
            scrollToBottom(messagesContainer);
        }
    }

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
            class="fixed inset-0 bg-black/50 z-40 md:hidden"
            on:click={() => (sidebarOpen = false)}
            aria-label="Close sidebar"
        ></button>
    {/if}

    <!-- Sidebar -->
    <div
        class={`fixed sm:relative z-50
        max-sm:left-0 max-sm:top-0 max-sm:h-full
        transition-all duration-300 
        ${sidebarOpen ? "max-sm:translate-x-0 w-80" : isMobile ? "w-80 -translate-x-full" : "w-0"} 
        overflow-hidden 
        border-r
        rounded-r-xl
        border-r-surface-200-800 
        body-background-color-dark
    `}
    >
        <div class="flex flex-col h-full">
            <!-- Sidebar Header -->
            <div class="p-4 border-b border-b-surface-200-800">
                <div class="flex items-center justify-between mb-4">
                    <div class="flex items-center gap-3">
                        <div class="preset-filled-primary-500 p-2 rounded-lg">
                            <Users size={20} />
                        </div>
                        <h2 class="text-lg font-semibold">Users</h2>
                    </div>
                    {#if isMobile}
                        <button
                            on:click={() => (sidebarOpen = false)}
                            class="preset-outlined-surface-200-800 p-2 rounded-lg hover:scale-105 transition-transform"
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
                                class="input w-full focus:ring-2 focus:ring-primary-500 rounded-lg text-sm"
                                required
                            />
                            <input
                                bind:value={AddedRecipientDisplayName}
                                type="text"
                                placeholder="Display name..."
                                class="input w-full focus:ring-2 focus:ring-primary-500 rounded-lg text-sm"
                                required
                            />
                        </div>
                        <button
                            type="submit"
                            class="btn px-3 preset-filled-surface-200-800 w-full rounded-lg hover:scale-105 active:scale-95 transition-transform disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100"
                            ><Plus size={20} /> Add recipient</button
                        >
                    </form>
                </div>
            </div>

            <!-- Users List -->
            <div class="flex-1 overflow-y-auto p-2">
                <div class="space-y-1">
                    {#each addedUsers as user}
                        <div
                            class={`w-full rounded-xl text-left hover:preset-filled-surface-100-900 transition-colors flex items-center justify-between ${recipient === user.id ? "preset-filled-primary-100-900 ring-2 ring-primary-500/30" : ""}`}
                        >
                            <button
                                on:click={() => selectRecipient(user.id)}
                                class="flex p-3 items-center gap-3 flex-1 text-left"
                            >
                                <div class="relative">
                                    {#if user.id == "server"}<div
                                            class="w-10 h-10 preset-filled-primary-400-500 flex items-center justify-center text-white font-medium"
                                        >
                                            <Server size="24" />
                                        </div>
                                    {:else}<div
                                            class="w-10 h-10 preset-filled-primary-400-500 rounded-full flex items-center justify-center text-white font-medium preset-outlined-surface-500"
                                        >
                                            {user.name.charAt(0)}
                                        </div>
                                    {/if}
                                </div>
                                <div class="flex-1 min-w-0">
                                    <p class="text-sm font-medium truncate">
                                        {user.name}
                                    </p>
                                </div>
                            </button>
                            <!-- If the user is not server, show the user controls 
                            like "remove from list" or "verify key"-->
                            {#if user.id != "server"}
                                <div class="pr-3 flex items-center">
                                    <button
                                        class="p-1 rounded hover:bg-surface-200-800"
                                        title="Verify authenticity of recipient key"
                                        on:click={() =>
                                            checkRecipientKey(user.id)}
                                    >
                                        {user.pk}
                                        <FileKey2 />
                                    </button>
                                    <button
                                        class="p-1 rounded hover:bg-surface-200-800"
                                        title="Remove a recipient from the list"
                                        on:click={() =>
                                            removeRecipient(user.id)}
                                    >
                                        <X />
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
                    class="w-full flex items-center gap-2 p-2 hover:preset-filled-surface-100-900 rounded-lg transition-colors opacity-70 hover:opacity-100"
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
            class="gap-2 grid-cols-[auto_1fr_auto] justify-between mt-2 mx-4 card items-stretch"
        >
            <div
                class="grid grid-cols-[auto_1fr_auto] gap-2 items-center preset-outlined-surface-200-800 card p-2"
            >
                <div class="flex items-center gap-2">
                    <button
                        on:click={toggleSidebar}
                        class="preset-outlined-surface-200-800 p-1 rounded hover:scale-105 transition-transform sm:hidden"
                    >
                        <Menu size={16} />
                    </button>
                    <button
                        on:click={toggleSidebar}
                        class="preset-outlined-surface-200-800 p-1 rounded hover:scale-105 transition-transform hidden sm:block"
                    >
                        <Users size={16} />
                    </button>
                </div>
                <div class="flex items-center gap-2 justify-center">
                    <Server size={20} />
                    <h1 class="text-sm sm:text-base truncate">
                        {$serverAddress}
                    </h1>
                </div>
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
            {#if messages.length === 0}
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
                {#each messages as msg, index}
                    <div
                        class="preset-filled-surface-100-900 card p-3 break-words"
                    >
                        <p class="text-sm leading-relaxed">{msg}</p>
                        <div class="text-xs opacity-50 mt-1">
                            {new Date().toLocaleTimeString([], {
                                hour: "2-digit",
                                minute: "2-digit",
                            })}
                        </div>
                    </div>
                {/each}
            {/if}
        </div>

        <!-- Message Input -->
        <div class="mx-4 mb-4">
            <form
                class="w-full card preset-outlined-surface-200-800 p-3 rounded-xl"
                on:submit|preventDefault={sendMessage}
            >
                <div class="grid grid-cols-[1fr_auto] gap-2">
                    <input
                        class="input rounded-lg focus:ring-2 focus:ring-primary-500"
                        type="text"
                        placeholder={recipient
                            ? `Message ${recipient}...`
                            : "Select a recipient first..."}
                        bind:value={message}
                        disabled={!recipient}
                        required
                    />

                    <button
                        class="btn px-3 preset-filled-surface-200-800 rounded-lg hover:scale-105 active:scale-95 transition-transform disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100"
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
