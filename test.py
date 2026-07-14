import sys
import re

content = open("src/events/popup_mouse.rs").read()
# find Template branch
match = re.search(r"Template\(mut p\).*?(?=\s*// === Group C4)", content, re.DOTALL)
if match:
    print(match.group(0))
