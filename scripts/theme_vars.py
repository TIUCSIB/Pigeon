# -*- coding: utf-8 -*-
import io

p = 'src/App.vue'
s = io.open(p, encoding='utf-8').read()

head, style = s.split('<style scoped>', 1)

pairs = [
    # card
    ('.card {\n  background: #fff;\n  border: 1px solid #e6e9ef;',
     '.card {\n  background: var(--card);\n  border: 1px solid var(--border);'),
    ('border-bottom: 1px solid #eef0f3;\n}\n.tabs button',
     'border-bottom: 1px solid var(--border-soft);\n}\n.tabs button'),
    ('.tabs button {\n  position: relative;\n  border: 0;\n  background: none;\n  padding: 0 0 13px;\n  font-size: 14px;\n  color: #8a93a8;',
     '.tabs button {\n  position: relative;\n  border: 0;\n  background: none;\n  padding: 0 0 13px;\n  font-size: 14px;\n  color: var(--text-3);'),
    ('.tabs button:hover {\n  color: #3b4557;\n}',
     '.tabs button:hover {\n  color: var(--text-2);\n}'),
    ('.tabs button.on {\n  color: #1a1d24;',
     '.tabs button.on {\n  color: var(--text);'),
    ('background: #1c6fd8;\n}\n.tab-n',
     'background: var(--accent);\n}\n.tab-n'),
    ('.tab-n {\n  font-size: 11px;\n  font-weight: 500;\n  color: #8a93a8;\n  background: #f2f4f7;',
     '.tab-n {\n  font-size: 11px;\n  font-weight: 500;\n  color: var(--text-3);\n  background: var(--chip-n);'),
    # hr
    ('.hr {\n  height: 1px;\n  background: #eef0f3;',
     '.hr {\n  height: 1px;\n  background: var(--border-soft);'),
    # labels
    ('.field label,\n.sec {\n  display: block;\n  font-size: 13px;\n  font-weight: 600;\n  color: #1a1d24;',
     '.field label,\n.sec {\n  display: block;\n  font-size: 13px;\n  font-weight: 600;\n  color: var(--text);'),
    ('.cap {\n  font-size: 13px;\n  font-weight: 600;\n  margin-bottom: 8px;\n}',
     '.cap {\n  font-size: 13px;\n  font-weight: 600;\n  margin-bottom: 8px;\n  color: var(--text);\n}'),
    ('.sub {\n  font-size: 12px;\n  color: #8a93a8;',
     '.sub {\n  font-size: 12px;\n  color: var(--text-3);'),
    # input
    ('.input {\n  width: 100%;\n  height: 40px;\n  border: 1px solid #e6e9ef;\n  background: #f7f8fa;',
     '.input {\n  width: 100%;\n  height: 40px;\n  border: 1px solid var(--border);\n  background: var(--field-bg);'),
    ('color: #1a1d24;\n  box-sizing: border-box;\n  user-select: text;',
     'color: var(--text);\n  box-sizing: border-box;\n  user-select: text;'),
    ('.input::placeholder {\n  color: #a8b0c0;\n}',
     '.input::placeholder {\n  color: var(--text-4);\n}'),
    ('.input:focus {\n  outline: none;\n  border-color: #1c6fd8;\n  background: #fff;\n}',
     '.input:focus {\n  outline: none;\n  border-color: var(--accent);\n  background: var(--card);\n}'),
    ('.input.readonly .lead {\n  width: 15px;\n  height: 15px;\n  flex: none;\n  color: #9aa3b5;\n}',
     '.input.readonly .lead {\n  width: 15px;\n  height: 15px;\n  flex: none;\n  color: var(--text-4);\n}'),
    ('.url {\n  font-family: ui-monospace, Consolas, monospace;\n  font-size: 12px;\n  color: #4a5468;',
     '.url {\n  font-family: ui-monospace, Consolas, monospace;\n  font-size: 12px;\n  color: var(--text-mono);'),
    # sq-btn
    ('.sq-btn {\n  flex: none;\n  width: 40px;\n  border: 1px solid #e6e9ef;\n  background: #fff;\n  border-radius: 9px;\n  color: #5b6478;',
     '.sq-btn {\n  flex: none;\n  width: 40px;\n  border: 1px solid var(--border);\n  background: var(--card);\n  border-radius: 9px;\n  color: var(--text-2);'),
    ('.sq-btn:hover:not(:disabled) {\n  border-color: #cfd6e2;\n  background: #f7f8fa;\n  color: #1a1d24;\n}',
     '.sq-btn:hover:not(:disabled) {\n  border-color: var(--border);\n  background: var(--accent-btn-hover);\n  color: var(--text);\n}'),
    ('.sq-btn:disabled {\n  color: #b6bdcc;\n  cursor: default;\n}',
     '.sq-btn:disabled {\n  color: var(--text-4);\n  cursor: default;\n}'),
    # btn
    ('.btn.primary {\n  background: #1c6fd8;\n  color: #fff;\n  font-weight: 500;\n}\n.btn.primary:hover:not(:disabled) {\n  background: #1a63c0;\n}\n.btn.primary:disabled {\n  background: #b9cdf0;',
     '.btn.primary {\n  background: var(--accent);\n  color: #fff;\n  font-weight: 500;\n}\n.btn.primary:hover:not(:disabled) {\n  background: var(--accent-hover);\n}\n.btn.primary:disabled {\n  background: var(--accent-bg);'),
    ('.btn.ghost {\n  background: #f2f4f7;\n  color: #3b4557;\n}\n.btn.ghost:hover {\n  background: #e9ecf1;\n}\n.btn.primary.danger {\n  background: #e5484d;\n}\n.btn.primary.danger:hover {\n  background: #d03a3f;\n}',
     '.btn.ghost {\n  background: var(--chip-bg);\n  color: var(--text-2);\n}\n.btn.ghost:hover {\n  background: var(--hover);\n}\n.btn.primary.danger {\n  background: var(--danger);\n}\n.btn.primary.danger:hover {\n  background: var(--danger);\n  opacity: 0.9;\n}'),
    # link
    ('.link {\n  border: 0;\n  background: none;\n  padding: 0;\n  color: #1c6fd8;',
     '.link {\n  border: 0;\n  background: none;\n  padding: 0;\n  color: var(--accent);'),
    # qr
    ('.qr {\n  width: 148px;\n  height: 148px;\n  border-radius: 10px;\n  border: 1px solid #eef0f3;',
     '.qr {\n  width: 148px;\n  height: 148px;\n  border-radius: 10px;\n  border: 1px solid var(--border-soft);\n  background: #fff;'),
    ('.qr-loading {\n  background: linear-gradient(100deg, #f0f2f6 40%, #fafbfd 50%, #f0f2f6 60%);',
     '.qr-loading {\n  background: linear-gradient(100deg, var(--field-bg) 40%, var(--hover) 50%, var(--field-bg) 60%);'),
    # drop zone
    ('.drop {\n  border: 1.5px dashed #d4dae4;',
     '.drop {\n  border: 1.5px dashed var(--border);'),
    ('.drop:hover,\n.drop.over {\n  border-color: #1c6fd8;\n  background: #f6f9ff;\n}',
     '.drop:hover,\n.drop.over {\n  border-color: var(--accent);\n  background: var(--accent-bg);\n}'),
    ('.dz-icon {\n  width: 38px;\n  height: 38px;\n  margin: 0 auto 9px;\n  border-radius: 50%;\n  background: #eef4ff;',
     '.dz-icon {\n  width: 38px;\n  height: 38px;\n  margin: 0 auto 9px;\n  border-radius: 50%;\n  background: var(--accent-bg);'),
    ('.dz-icon svg {\n  width: 19px;\n  height: 19px;\n  fill: #1c6fd8;\n}',
     '.dz-icon svg {\n  width: 19px;\n  height: 19px;\n  fill: var(--accent);\n}'),
    ('.dz-t {\n  font-size: 13.5px;\n  font-weight: 600;\n}',
     '.dz-t {\n  font-size: 13.5px;\n  font-weight: 600;\n  color: var(--text);\n}'),
    ('.dz-s {\n  font-size: 12px;\n  color: #8a93a8;',
     '.dz-s {\n  font-size: 12px;\n  color: var(--text-3);'),
    # list
    ('.sec-n {\n  font-size: 12px;\n  color: #a2aab9;\n}',
     '.sec-n {\n  font-size: 12px;\n  color: var(--text-4);\n}'),
    ('.list li {\n  display: flex;\n  align-items: center;\n  gap: 4px;\n  border-bottom: 1px solid #f1f3f6;\n}',
     '.list li {\n  display: flex;\n  align-items: center;\n  gap: 4px;\n  border-bottom: 1px solid var(--border-row);\n}'),
    ('.row-main:hover {\n  background: #fafbfd;\n}',
     '.row-main:hover {\n  background: var(--hover);\n}'),
    ('.ext {\n  flex: none;\n  width: 36px;\n  height: 36px;\n  border-radius: 9px;\n  background: #f2f4f8;\n  color: #5b6478;',
     '.ext {\n  flex: none;\n  width: 36px;\n  height: 36px;\n  border-radius: 9px;\n  background: var(--chip-bg);\n  color: var(--text-2);'),
    ('.row-name {\n  font-size: 13px;\n  color: #1a1d24;',
     '.row-name {\n  font-size: 13px;\n  color: var(--text);'),
    ('.row-sub {\n  font-size: 11.5px;\n  color: #8a93a8;\n}',
     '.row-sub {\n  font-size: 11.5px;\n  color: var(--text-3);\n}'),
    ('.op {\n  border: 0;\n  background: none;\n  width: 28px;\n  height: 28px;\n  border-radius: 7px;\n  color: #9aa3b5;',
     '.op {\n  border: 0;\n  background: none;\n  width: 28px;\n  height: 28px;\n  border-radius: 7px;\n  color: var(--text-3);'),
    ('.op:hover {\n  background: #eef4ff;\n  color: #1c6fd8;\n}\n.op.danger:hover {\n  background: #fdeeee;\n  color: #e5484d;\n}',
     '.op:hover {\n  background: var(--accent-bg);\n  color: var(--accent);\n}\n.op.danger:hover {\n  background: var(--danger-bg);\n  color: var(--danger);\n}'),
    ('.clip-text {\n  font-size: 13px;\n  line-height: 1.55;\n  color: #1a1d24;',
     '.clip-text {\n  font-size: 13px;\n  line-height: 1.55;\n  color: var(--text);'),
    ('.clip-copy {\n  flex: none;\n  align-self: center;\n  font-size: 11.5px;\n  color: #1c6fd8;\n  background: #eef4ff;',
     '.clip-copy {\n  flex: none;\n  align-self: center;\n  font-size: 11.5px;\n  color: var(--accent);\n  background: var(--accent-bg);'),
    ('.empty {\n  text-align: center;\n  color: #a2aab9;',
     '.empty {\n  text-align: center;\n  color: var(--text-4);'),
    ('border-top: 1px solid #f1f3f6;\n}\n.path {\n  font-size: 11px;\n  font-family: ui-monospace, Consolas, monospace;\n  color: #b0b8c9;',
     'border-top: 1px solid var(--border-row);\n}\n.path {\n  font-size: 11px;\n  font-family: ui-monospace, Consolas, monospace;\n  color: var(--text-4);'),
    # note code
    ('.note code {\n  font-family: ui-monospace, Consolas, monospace;\n  background: #f2f4f7;\n  border-radius: 4px;\n  padding: 1px 5px;\n  color: #5b6478;\n}',
     '.note code {\n  font-family: ui-monospace, Consolas, monospace;\n  background: var(--chip-bg);\n  border-radius: 4px;\n  padding: 1px 5px;\n  color: var(--text-2);\n}'),
    # status
    ('.status {\n  font-size: 12px;\n  color: #5b6478;',
     '.status {\n  font-size: 12px;\n  color: var(--text-2);'),
    ('background: #22a06b;\n}',
     'background: var(--ok);\n}'),
    # brand-name
    ('.brand-name {\n  font-size: 15px;\n  font-weight: 700;\n  letter-spacing: 0.2px;\n}',
     '.brand-name {\n  font-size: 15px;\n  font-weight: 700;\n  letter-spacing: 0.2px;\n  color: var(--text);\n}'),
    # toast
    ('.toast {\n  background: #1a1d24;',
     '.toast {\n  background: var(--toast-bg);'),
    # drag overlay
    ('.drag-overlay {\n  position: fixed;\n  inset: 0;\n  background: rgba(28, 111, 216, 0.07);',
     '.drag-overlay {\n  position: fixed;\n  inset: 0;\n  background: var(--drag-tint);'),
    ('.drag-box {\n  background: #fff;',
     '.drag-box {\n  background: var(--card);'),
    ('.drag-t {\n  font-size: 14px;\n  font-weight: 700;\n  color: #1c6fd8;\n}',
     '.drag-t {\n  font-size: 14px;\n  font-weight: 700;\n  color: var(--accent);\n}'),
    ('.drag-s {\n  font-size: 12px;\n  color: #8a93a8;\n  margin-top: 4px;\n}',
     '.drag-s {\n  font-size: 12px;\n  color: var(--text-3);\n  margin-top: 4px;\n}'),
    # modal
    ('.modal-mask {\n  position: fixed;\n  inset: 0;\n  background: rgba(26, 29, 36, 0.38);',
     '.modal-mask {\n  position: fixed;\n  inset: 0;\n  background: var(--mask);'),
    ('.modal {\n  background: #fff;',
     '.modal {\n  background: var(--card);'),
    ('.modal-t {\n  font-size: 15px;\n  margin-bottom: 10px;\n}',
     '.modal-t {\n  font-size: 15px;\n  margin-bottom: 10px;\n  color: var(--text);\n}'),
    ('.modal-desc {\n  font-size: 13px;\n  color: #5b6478;',
     '.modal-desc {\n  font-size: 13px;\n  color: var(--text-2);'),
    ('.modal-desc b {\n  color: #1a1d24;\n  word-break: break-all;\n}',
     '.modal-desc b {\n  color: var(--text);\n  word-break: break-all;\n}'),
]

count = 0
for old, new in pairs:
    if old in style:
        style = style.replace(old, new)
        count += 1
    else:
        first_line = old.split('\n')[0][:70]
        print('MISS:', first_line)

io.open(p, 'w', encoding='utf-8', newline='\n').write(head + '<style scoped>' + style)
print('replaced %d/%d' % (count, len(pairs)))
