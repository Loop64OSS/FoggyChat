import sys
import os
import asyncio
from typing import Optional
from dataclasses import dataclass

@dataclass
class FctpMessage:
    code: int
    from_: str
    body: str
    to: str

def decapsulate_fctp_message(msg: str) -> Optional[FctpMessage]:
    lines = msg.split('\n')
    line_iter = iter(lines)
    
    try:
        if next(line_iter) != "FoggyChat Transfer Protocol 0.1":
            return None
        
        code_str = next(line_iter).strip()
        try:
            code = int(code_str)  # ABSOLUTELY REQUIRED, MUST BE A NUMBER IN INT FORMAT 32 BIT SIZE
        except ValueError:
            return None
        
        from_line = next(line_iter)
        if not from_line.startswith("From: "):
            return None
        from_ = from_line[6:].strip()  # Remove "From: " prefix
        
        body_line = next(line_iter)
        if not body_line.startswith("Body: "):
            return None
        body = body_line[6:].strip()  # Remove "Body: " prefix
        
        to_line = next(line_iter)
        if not to_line.startswith("To: "):
            return None
        to = to_line[4:].strip()  # Remove "To: " prefix, To: is optional, but we keep it for consistency. TL/DR: ignored
        
        return FctpMessage(code=code, from_=from_, body=body, to=to)
    
    except StopIteration:
        return None

def encapsulate_to_fctp(code: int, from_: str, body: str, to: str) -> str:
    return f"FoggyChat Transfer Protocol 0.1\r\n{code}\r\nFrom: {from_}\r\nBody: {body}\r\nTo: {to}\r\n\r\n"

async def process_fctp_stream(message: str):
    msg = decapsulate_fctp_message(message)
    if msg is not None:
        #pool 4xx - client-side errors
        #pool 5xx - server-side errors
        #pool 2xx - message handling
        #pool 1x - connection handling (ping/pong) TODO: pong
        #pool 8xx - encryption handshake TODO
        #pool 9xx - logon information ex. logged in, session info TODO
        if msg.code == 200:
            print(f"[Message received] <{msg.from_}> {msg.body}")  #User-user direct message
        elif msg.code == 201:
            print(f" {msg.from_}: {msg.body}")  #From-server general direct message
        elif msg.code == 405:
            print(f"[!cl!] {msg.from_}: {msg.body}")  #Client-side error
        elif msg.code == 505:
            print(f"[!sv!] {msg.from_}: {msg.body}")  #Server-side error
        elif msg.code == 11:
            pass  #Pong handling (ping code 10, pong code 11), TODO: connection keep-alive
        else:
            print(f"[!Unsupported code!]: {msg.code}\n !Update your client software or ask server administrator to update his server software!")  #Unknown code handling
    else:
        print(f"[!Malformed header!]: {message}", file=sys.stderr)

async def main():
    """
       Main code Logic
    """
    
    # start stream #TODO: SOCKS5stream support and address selection in user input
    reader, writer = await asyncio.open_connection('127.0.0.1', 8081)
    
    # Create async queue for message passing
    queue = asyncio.Queue(maxsize=100)
    
    print()
    
    async def receive_messages():
        #receive messages from server and process them
        message = ""
        # Glue the lines together
        try:
            while True:
                line = await reader.readline()
                if not line:
                    break
                line = line.decode('utf-8').rstrip('\n\r')
                
                if line.strip() == "" and message != "":
                    if message.endswith('\n'):
                        message = message[:-1]
                        message += "\r\n\r\n"
                    await process_fctp_stream(message)
                    message = ""
                else:
                    message += line
                    message += '\n'
        except Exception:
            pass
        print("CON CLOSED - Disconnected")
        sys.exit(0)
    
    async def send_messages():
        #receive messages from async channel and send them to server - universal sender
        try:
            while True:
                msg = await queue.get()
                writer.write(msg.encode('utf-8'))
                await writer.drain()
        except Exception as e:
            print(f"[!Couldn't send the message!]: {e}", file=sys.stderr)
    
    async def handle_stdin():
        # stdin handle messaging,commands etc. and send it to server
        loop = asyncio.get_event_loop()
        try:
            while True:
                line = await loop.run_in_executor(None, input)
                
                # Split line by ':' and extract parts
                parts = line.split(':', 1)
                to_part = parts[0].strip() if len(parts) > 0 else ""
                body_part = parts[1].strip() if len(parts) > 1 else ""
                
                packet = encapsulate_to_fctp(200, "me", body_part, to_part)
                
                await queue.put(packet + "\n")
        except EOFError:
            pass
        except Exception:
            pass
    
    async def ping_server():
        # Ping server every 2 seconds with code 10
        try:
            while True:
                await asyncio.sleep(2)
                ping_packet = encapsulate_to_fctp(10, "me", "ping", "server")
                await queue.put(ping_packet)
        except Exception:
            pass
    
    # Spawn all tasks
    asyncio.create_task(receive_messages())
    asyncio.create_task(send_messages())
    asyncio.create_task(handle_stdin())
    asyncio.create_task(ping_server())
    
    # Main loop
    while True:
        await asyncio.sleep(60)

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print('Interrupted')
        try:
            sys.exit(130)
        except SystemExit:
            os._exit(130)
