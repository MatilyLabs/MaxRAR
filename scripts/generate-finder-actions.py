#!/usr/bin/env python3
"""Generate source templates; native installation supplies the app location."""
import plistlib
from pathlib import Path
for mode,title in [('compress','Compress with MaxRAR…'),('extract','Extract with MaxRAR…')]:
    directory=Path('src-tauri/shell/macos')/mode
    directory.mkdir(parents=True,exist_ok=True)
    service={'NSSendFileTypes':['public.item'],'NSMenuItem':{'default':title},'NSMessage':'runWorkflowAsService','NSRequiredContext':{'NSApplicationIdentifier':'com.apple.finder'}}
    info={'NSServices':[service],'MaxRARManaged':'app.maxrar.desktop.shell.v1'}
    action={'AMAccepts':{'Container':'List','Optional':False,'Types':['com.apple.cocoa.string']},'AMProvides':{'Container':'List','Types':['com.apple.cocoa.string']},'ActionBundlePath':'/System/Library/Automator/Run Shell Script.action','ActionName':'Run Shell Script','ActionParameters':{'COMMAND_STRING':'/usr/bin/open -n -a __MAXRAR_APP__ --args --'+mode+' -- "$@"','CheckedForUserDefaultShell':True,'inputMethod':1,'shell':'/bin/sh','source':''},'BundleIdentifier':'com.apple.RunShellScript','CFBundleVersion':'2.0','CanShowSelectedItemsWhenRun':False,'CanShowWhenRun':False,'Category':['AMCategoryUtilities'],'Class Name':'RunShellScriptAction','InputUUID':'9F52B791-E348-4F46-BE98-5D332777D911','OutputUUID':'42BC463D-10D0-4EC0-BC30-92AB36454B98','UUID':'9F52B791-E348-4F46-BE98-5D332777D912','isViewVisible':1}
    metadata={'serviceApplicationBundleID':'com.apple.finder','serviceApplicationPath':'/System/Library/CoreServices/Finder.app','serviceInputTypeIdentifier':'com.apple.Automator.fileSystemObject','serviceOutputTypeIdentifier':'com.apple.Automator.nothing','serviceProcessesInput':0,'workflowTypeIdentifier':'com.apple.Automator.servicesMenu'}
    workflow={'AMApplicationBuild':'523','AMApplicationVersion':'2.10','AMDocumentVersion':'2','actions':[{'action':action,'isViewVisible':1}],'connectors':{},'workflowMetaData':metadata}
    for name,data in [('Info.plist',info),('document.wflow',workflow)]:
        (directory/name).write_bytes(plistlib.dumps(data))
