import base64,hashlib,sys,glob,re
b=''.join(re.sub(r'\s','',open(f).read().replace('[CHUNK','').replace('[END','') and re.sub(r'\[.*?\]','',open(f).read())) for f in sorted(glob.glob('chat_*.txt')))
d=base64.b64decode(b);open('MouseTweaks-RS_v1.0.0.hmod','wb').write(d)
h=hashlib.sha256(d).hexdigest();ok=h=='7b619bc4a08342730268d8dc473c104f44178bf42d8c96c661901361aff51298'
print(len(d),'bytes; sha256',h[:16],'MATCH OK' if ok else 'MISMATCH!!!')