-- Real CMD/Clink integration: separate submitted lines, no mocked environment.
local source=debug.getinfo(1,'S').source:sub(2):gsub('\\','/')
local root=assert(source:match('^(.*)/tests/[^/]+$'))
local work=root..'/target/test-work'
local fixture="'"..work.."/fixture.exe'"
local adapter=_G.clink_posix_native_api or dofile(root..'/native-preview.lua')
local report=assert(io.open(root..'/clink-verification.txt','wb'))
local original_cwd=os.getcwd()
local child=work..'/中文 & percent% bang! caret^'
os.mkdir(child)
local function check(ok,label)
    report:write((ok and 'PASS ' or 'FAIL ')..label..'\n');report:flush()
end
local function read(name)
    local f=io.open(work..'/'..name,'rb');if not f then return nil end
    local s=f:read('*a');f:close();return s
end
local function output(line,name) return line.." > '"..work..'/'..name.."'" end
local steps={
    function() return output("px echo '中文 & %PATH%'","literal") end,
    function() check(read('literal')=='中文 & %PATH%\n','quoted UTF-8 literal');return 'px false' end,
    function() check(tonumber(os.getenv('ERRORLEVEL'))==1,'nonzero exit code');return output('px echo $?','status') end,
    function() check(read('status')=='1\n','prior exit status');return "px export CPX_PARITY='中文 & %PATH% ! ^'; cd '"..child.."' ; false" end,
    function()
        check(os.getcwd():gsub('\\','/'):lower()==child:lower(),'CMD cwd persists with Unicode and metacharacters')
        check(os.getenv('CPX_PARITY')=='中文 & %PATH% ! ^','environment persists literally')
        check(tonumber(os.getenv('ERRORLEVEL'))==1,'cwd sync preserves nonzero status')
        return output('px echo "$CPX_PARITY" "$PWD"','nextline')
    end,
    function()
        local value=read('nextline') or ''
        check(value:find('中文 & %PATH% ! ^',1,true)==1 and value:gsub('\\','/'):find(child,1,true)~=nil,'next request inherits env and cwd')
        return 'px unset CPX_PARITY'
    end,
    function() check(os.getenv('CPX_PARITY')==nil,'unset persists');return 'px cmd doskey cpxsay=echo $*' end,
    function() return output('px cpxsay "a & b"','alias') end,
    function() check(read('alias')=='a & b\n','DOSKEY alias arguments');return 'px auto' end,
    function() return output("echo {a,b} $((2+3))","auto") end,
    function()
        check(read('auto')=='a b 5\n','automatic syntax routing')
        check(adapter.enhanced('set CPX_NATIVE=value')==false,'CMD builtin bypass')
        return 'px off'
    end,
    function() return 'echo $CPX_LITERAL > "'..work..'/off"' end,
    function() check((read('off') or ''):find('$CPX_LITERAL',1,true)~=nil,'off retains CMD semantics');return output('px echo forced','forced') end,
    function() check(read('forced')=='forced\n','explicit px while off');return 'px on' end,
    function() return output('echo \'on mode\'','on') end,
    function() check(read('on')=='on mode\n','on mode routes ordinary commands');return 'px cmd echo $LITERAL > "'..work..'/cmd"' end,
    function() check((read('cmd') or ''):find('$LITERAL',1,true)~=nil,'explicit CMD bypass while on');return 'px auto' end,
    function() return output("px "..fixture.." printf '%s\\n' b a | "..fixture.." sort",'pipeline') end,
    function() check(read('pipeline')=='a\nb\n','concurrent external pipeline');return "px "..fixture.." diff <(echo same) <(echo same)" end,
    function() check(tonumber(os.getenv('ERRORLEVEL'))==0,'finite process substitution');return "px echo '" end,
    function() check(tonumber(os.getenv('ERRORLEVEL'))==2,'parse failure preserves exit status');return output('nx echo alias','nx') end,
    function() check(read('nx')=='alias\n','nx compatibility alias');return "px cd '"..original_cwd.."'" end,
    function() check(os.getcwd():lower()==original_cwd:lower(),'return to original cwd');report:close();return 'px cmd exit' end,
}
local index=0
clink.onprovideline(function()
    index=index+1
    if steps[index] then return steps[index]() end
    return 'exit'
end)
