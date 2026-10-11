-- Loaded only by the isolated smoke-test profile. Runs and exits automatically.
local source = debug.getinfo(1,'S').source:sub(2):gsub('\\','/')
local root = assert(source:match('^(.*)/tests/[^/]+$'))
local work = root .. '/target/test-work'
local fixture = "'" .. work .. "/fixture.exe'"
dofile(root .. '/native-preview.lua')
local report = assert(io.open(root .. '/clink-verification.txt','wb'))
local original_cwd, original_env = os.getcwd(), os.getenv('CLINK_NATIVE_SMOKE')
local step = 0
local function check(ok,label)
    report:write((ok and 'PASS ' or 'FAIL ')..label..'\n'); report:flush()
end
local function content(name)
    local f = io.open(work..'/'..name,'rb')
    if not f then return nil end
    local value=f:read('*a'); f:close(); return value
end
clink.onprovideline(function()
    step=step+1
    if step==1 then return "nx echo '中文 & %PATH%' > '"..work.."/native-output.txt'"
    elseif step==2 then
        check(content('native-output.txt')=='中文 & %PATH%\n','UTF-8 request and literal CMD metacharacters')
        return 'nx false'
    elseif step==3 then
        check(tonumber(os.getenv('ERRORLEVEL'))==1,'native exit status preserved without sync batch')
        return "nx echo $? > '"..work.."/native-status.txt'"
    elseif step==4 then
        check(content('native-status.txt')=='1\n','previous CMD exit status passed into native executor')
        return "nx export CLINK_NATIVE_SMOKE=child; cd '"..root.."'; echo $CLINK_NATIVE_SMOKE > '"..work.."/native-env.txt'"
    elseif step==5 then
        check(content('native-env.txt')=='child\n','state shared within one submitted line')
        check(os.getcwd()==original_cwd and os.getenv('CLINK_NATIVE_SMOKE')==original_env,'parent state intentionally unchanged in preview')
        return "nx "..fixture.." printf '%s\\n' b a | "..fixture.." sort > '"..work.."/native-pipeline.txt'"
    elseif step==6 then
        check(content('native-pipeline.txt')=='a\nb\n','concurrent pipeline through real Clink adapter')
        return "nx "..fixture.." diff <("..fixture.." printf 'same\\n') <("..fixture.." printf 'same\\n')"
    elseif step==7 then
        check(tonumber(os.getenv('ERRORLEVEL'))==0,'finite process substitution through real Clink adapter')
        report:close(); return 'exit'
    end
    return 'exit'
end)
