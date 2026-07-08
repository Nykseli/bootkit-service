
import json

def dbus_call(machine, interface: str, mehtod: str, is_json: bool):
    djson = machine.execute(f"busctl call --json=short org.opensuse.bootkit /org/opensuse/bootkit org.opensuse.bootkit.{interface} {mehtod}").strip()
    data = json.loads(djson)["data"][0]
    if is_json:
        return json.loads(data)
    else:
        return data
