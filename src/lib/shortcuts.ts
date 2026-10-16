export const commandShortcuts=[{id:'open',key:'o',label:'Open archive'},{id:'create',key:'n',label:'Create archive'},{id:'find',key:'f',label:'Find files'},{id:'select_all',key:'a',label:'Select all filtered files'}] as const;
export function shortcutAction(event:{key:string;ctrlKey:boolean;metaKey:boolean;altKey:boolean;shiftKey:boolean}){
 if(!(event.ctrlKey||event.metaKey)||event.altKey||event.shiftKey)return undefined;
 return commandShortcuts.find(shortcut=>shortcut.key===event.key.toLowerCase())?.id;
}
