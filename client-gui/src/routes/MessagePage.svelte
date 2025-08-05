<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
    var message = "";
    function sendMessage() {
        invoke("send_message", { line: message });
    }
    let messages: string[] = [];
    function addMessage(text: string) {
        messages = [...messages, text];
    }
    const appWebview = getCurrentWebviewWindow();
    appWebview.listen<string>("pass-message", (event) => {
        addMessage(event.payload);
        console.log(event.payload);
    });
</script>

<main>
    <form on:submit|preventDefault={sendMessage}>
        <div class="w-full p-10 card">
            <div class="input-group grid-cols-[1fr_auto]">
                <input
                    class="ig-input"
                    type="text"
                    placeholder="Input"
                    bind:value={message}
                />
                <button class="ig-btn" type="submit">Send</button>
            </div>
        </div>
    </form>

    <div id="message-container" style="overflow-y: scroll; height:400px;">
        {#each messages as msg}
            <p>{msg}</p>
        {/each}
    </div>
</main>
