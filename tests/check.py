"""Black-box conformance and shared-subset differential checks; no pip packages."""
import argparse, os, subprocess, sys, tempfile, unittest
from pathlib import Path
from support import ROOT, WORK, EXE, FIXTURE, MANIFEST, native_args, prepare_fixture

OLD = None
F = "'" + FIXTURE.as_posix() + "'"

class Checks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='clink-native-test-')
        self.addCleanup(self.temp.cleanup)
        self.cwd = Path(self.temp.name)
    def runline(self, line, *, old=False, **kwargs):
        command = [sys.executable, str(OLD)] if old else native_args()
        return subprocess.run(command + ['-c', line], cwd=self.cwd, input=b'', capture_output=True, timeout=20, **kwargs)
    def same(self,line,expected=None,code=0):
        native=self.runline(line)
        self.assertEqual(native.returncode,code,native.stderr)
        if OLD:
            old=self.runline(line,old=True)
            self.assertEqual((native.returncode,native.stdout),(old.returncode,old.stdout),(line,native.stderr,old.stderr))
        if expected is not None:self.assertEqual(native.stdout,expected)
        return native
    def test_quotes_argv(self):
        values=['a b','a"b','%PATH%','a&b','','中文',r'C:\new\test','x\\']
        line=F+' args '+ ' '.join("'"+s+"'" for s in values)
        p=self.same(line)
        self.assertEqual([bytes.fromhex(s).decode() for s in p.stdout.decode().splitlines()],values)
    def test_quoted_adjacency(self):self.same(F+" args a''b a\\ b",b'6162\n612062\n')
    def test_substitution_no_reparse(self):self.same('echo "$(echo A\\&B)"',b'A&B\n')
    def test_substitution_literal(self):self.same("echo '$(echo nope)'",b'$(echo nope)\n')
    def test_nested_substitution(self):self.same('echo "$(echo "$(echo nested)")"',b'nested\n')
    def test_trim_substitution(self):self.same('echo "$(printf \'a\\n\\n\')"',b'a\n')
    def test_short_circuit(self):
        self.same('false && echo $(echo bad > marker); true || echo $(echo bad > marker)',b'')
        self.assertFalse((self.cwd/'marker').exists())
    def test_status(self):self.same('false; echo $?; true && echo yes || echo no',b'1\nyes\n')
    def test_status_inherited(self):
        p=subprocess.run([str(EXE),'--status','9','-c','echo $?'],capture_output=True,timeout=10)
        self.assertEqual(p.stdout,b'9\n')
    def test_ordered_assignments(self):self.same('CPX_A=hello CPX_B=$CPX_A; echo "$CPX_B"',b'hello\n')
    def test_assignment_scope(self):self.same('CPX_A=old; CPX_A=new '+F+' env CPX_A; echo "$CPX_A"',b'new\nold\n')
    def test_env_literal(self):self.same("CPX_A='a&echo BAD %PATH% $(echo BAD)'; echo \"$CPX_A\"",b'a&echo BAD %PATH% $(echo BAD)\n')
    def test_empty_unquoted(self):self.same(F+' args $CLINK_NATIVE_NONEXIST "${CLINK_NATIVE_NONEXIST}"',b'\n')
    def test_field_splitting(self):self.same("CPX_A='  a   b  '; "+F+' args $CPX_A "$CPX_A"',b'61\n62\n202061202020622020\n')
    def test_export_unset(self):self.same('export CPX_A=hello; '+F+' env CPX_A; unset CPX_A; echo "$CPX_A"',b'hello\n\n')
    def test_cd_order(self):
        (self.cwd/'child').mkdir()
        self.same('cd child; echo "$(pwd)"',(str(self.cwd/'child')+'\n').encode())
    def test_capture_scope(self):self.same('CPX_A=outer; echo "$(CPX_A=inner; echo $CPX_A)"; echo $CPX_A',b'inner\nouter\n')
    def test_pipeline_scope(self):self.same('CPX_A=outer; CPX_A=inner | cat; echo $CPX_A',b'outer\n')
    def test_tools(self):self.same("printf '%s\\n' b a | sort",b'a\nb\n')
    def test_large_pipe(self):self.same(F+' emit 4194304 | '+F+' count',b'4194304\n')
    def test_binary_pipe(self):self.same(F+' emit 262144 | '+F+' cat',bytes(range(256))*1024)
    def test_pipe_early_exit(self):self.same(F+' emit 16777216 | '+F+' first',b'\0')
    def test_pipe_status(self):self.same('false | true',b'')
    def test_merged_pipe(self):self.same(F+' both |& cat',b'out\nerr\n')
    def test_redirect_order(self):
        self.same(F+' both 2>&1 > log',b'err\n')
        self.assertEqual((self.cwd/'log').read_bytes(),b'out\n')
    def test_redirect_merge(self):
        self.same(F+' both > log 2>&1',b'')
        self.assertEqual((self.cwd/'log').read_bytes(),b'out\nerr\n')
    def test_append_unicode(self):
        self.same("echo '中文' > '中文 文件'; echo more >> '中文 文件'; cat < '中文 文件'",'中文\nmore\n'.encode())
    def test_here_string(self):self.same("cat <<< 'a&b %PATH%'",b'a&b %PATH%\n')
    def test_null(self):self.same('echo hidden > /dev/null',b'')
    def test_process_substitution(self):self.same("diff <(printf 'same\\n') <(printf 'same\\n')",b'')
    def test_process_binary(self):self.same('cat <('+F+' emit 262144)',bytes(range(256))*1024)
    def test_temp_cleanup(self):
        p=self.runline('echo <(echo hello)')
        self.assertEqual(p.returncode,0,p.stderr)
        self.assertFalse(Path(p.stdout.decode().strip()).exists())
    def test_missing_recovery(self):self.same('clink_native_missing_command || echo recovered',b'recovered\n')
    def test_exitcode(self):self.same(F+' exit 7',b'',7)
    def test_static_rejection_before_side_effect(self):
        for bad in ['source foo','echo "oops','echo hi &','echo $((2**3))','echo ${A:=x}','(echo x','echo $(source foo)','echo `pwd`']:
            with self.subTest(bad=bad):
                p=self.runline('echo bad > marker; '+bad)
                self.assertEqual(p.returncode,2,p.stderr)
                self.assertFalse((self.cwd/'marker').exists(),bad)
    def test_batch_rejection(self):
        p=self.runline('example.cmd')
        self.assertEqual(p.returncode,2)
        self.assertIn(b'Batch files',p.stderr)
    def test_capture_limit(self):
        p=self.runline('echo "$('+F+' emit 8388609)"')
        self.assertEqual(p.returncode,2,p.stderr)
        self.assertIn(b'8 MiB',p.stderr)
    def test_non_utf8_capture(self):
        p=self.runline('echo "$('+F+' emit 256)"')
        self.assertEqual(p.returncode,2,p.stderr)
        self.assertIn(b'UTF-8',p.stderr)
    def test_nesting_limit(self):
        p=self.runline('echo '+ '$(echo '*18+'x'+')'*18)
        self.assertEqual(p.returncode,2)
    def test_state_protocol(self):
        output=self.cwd/'state'; (self.cwd/'中文 child').mkdir()
        p=subprocess.run([str(EXE),'--state',str(output),'-c',"export CPX_A='中文 & %'; cd '中文 child'; false"],cwd=self.cwd,capture_output=True,timeout=10)
        self.assertEqual(p.returncode,1,p.stderr)
        lines=output.read_text().splitlines()
        self.assertIn('V\t1',lines);self.assertIn('R\t1',lines)
        self.assertIn('C\t'+str(self.cwd/'中文 child').encode().hex(),lines)
        self.assertIn('S\t'+b'CPX_A'.hex()+'\t'+'中文 & %'.encode().hex(),lines)
    def test_request_file(self):
        request=self.cwd/'request';request.write_text('echo 中文',encoding='utf8')
        p=subprocess.run([str(EXE),'--request',str(request)],capture_output=True,timeout=10)
        self.assertEqual(p.stdout,'中文\n'.encode())
    def test_request_limit(self):
        request=self.cwd/'long';request.write_text('echo '+ 'x'*65536)
        p=subprocess.run([str(EXE),'--request',str(request)],capture_output=True,timeout=10)
        self.assertEqual(p.returncode,2)
        self.assertIn(b'64 KiB',p.stderr)
    def test_multiline(self):
        self.same("echo 'a\nb'\necho c # comment\necho d",b'a\nb\nc\nd\n')
    def test_line_continuation(self):
        self.same("echo a\\\nb",b'ab\n')
    def test_manifest_rejects_batch(self):
        manifest=self.cwd/'tools.tsv';manifest.write_text('foo\tC:/example.cmd\n')
        p=subprocess.run([str(EXE),'--tools',str(manifest),'-c','echo should-not-run'],capture_output=True,timeout=10)
        self.assertEqual(p.returncode,2)
        self.assertEqual(p.stdout,b'')
    def test_temp_cleanup_on_failure(self):
        tempdir=self.cwd/'temp';tempdir.mkdir()
        env=dict(os.environ,TEMP=str(tempdir),TMP=str(tempdir))
        p=self.runline('clink_native_missing <(echo hello)',env=env)
        self.assertEqual(p.returncode,127)
        self.assertEqual(list(tempdir.iterdir()),[])
    def test_syntax_error_does_not_publish_state(self):
        output=self.cwd/'state'
        p=subprocess.run([str(EXE),'--state',str(output),'-c',"echo before; echo '"],capture_output=True,timeout=10)
        self.assertEqual(p.returncode,2)
        self.assertFalse(output.exists())
        self.assertEqual(p.stdout,b'')
    def test_runtime_has_no_scoop_dependency(self):
        env=dict(os.environ,PATH=os.path.join(os.environ['SystemRoot'],'System32'))
        p=self.runline('echo hello',env=env)
        self.assertEqual((p.returncode,p.stdout),(0,b'hello\n'),p.stderr)

    def test_brace_lists(self): self.same('echo pre{a,b}{1,2}post',b'prea1post prea2post preb1post preb2post\n')
    def test_brace_nested(self): self.same('echo {a,{b,c}}',b'a b c\n')
    def test_brace_ranges(self): self.same('echo {-2..2} {5..1..2}',b'-2 -1 0 1 2 5 3 1\n')
    def test_brace_quotes(self): self.same('echo "{a,b}" \\{a,b\\}',b'{a,b} {a,b}\n')
    def test_brace_limit(self):
        p=self.runline('echo {1..999999999}')
        self.assertEqual(p.returncode,2,p.stderr)
    def test_assignment_no_brace_split_glob(self): self.same("CPX_A={a,b}; echo \"$CPX_A\"",b'{a,b}\n')
    def test_tilde(self):
        env=dict(os.environ,HOME=str(self.cwd/'a [b]'))
        p=self.runline('echo ~ ~/child "~"',env=env)
        self.assertEqual(p.stdout,(env['HOME']+' '+env['HOME']+'/child ~\n').encode())
    def test_glob(self):
        for name in ['b.txt','a.txt','.hidden.txt','other']: (self.cwd/name).touch()
        self.same('echo *.txt "*.txt" missing*',b'a.txt b.txt *.txt missing*\n')
    def test_glob_class(self):
        for name in ['a.txt','b.txt','c.txt']: (self.cwd/name).touch()
        self.same('echo [a-b].txt [!ab].txt ?.txt',b'a.txt b.txt c.txt a.txt b.txt c.txt\n')
    def test_glob_nested(self):
        for name in ['a','b']:
            (self.cwd/name).mkdir();(self.cwd/name/'x.txt').touch()
        p=self.runline('echo */*.txt')
        self.assertEqual(p.stdout,b'a/x.txt b/x.txt\n')
    def test_glob_quoted_prefix(self):
        (self.cwd/'[a]').mkdir(); (self.cwd/'[a]'/'x.txt').touch()
        p=self.runline('echo "[a]"/*.txt')
        self.assertEqual(p.stdout,b'[a]/x.txt\n')
        if OLD:
            old=self.runline('echo "[a]"/*.txt',old=True)
            self.assertEqual(old.stdout.replace(b'\\',b'/'),p.stdout)
    def test_glob_dynamic(self):
        (self.cwd/'x.txt').touch()
        self.same("CPX_A='*.txt'; echo $CPX_A \"$CPX_A\"",b'x.txt *.txt\n')
    def test_glob_ambiguous_redirect(self):
        for name in ['a.txt','b.txt']: (self.cwd/name).write_bytes(b'keep')
        p=self.runline('echo lost > *.txt')
        self.assertEqual(p.returncode,2,p.stderr)
        self.assertEqual((self.cwd/'a.txt').read_bytes(),b'keep')
    def test_recursive_glob_rejected(self):
        self.assertEqual(self.runline('echo **/*.txt').returncode,2)
    def test_parameter_defaults(self):
        self.same('unset CPX_A; echo "${CPX_A-x}" "${CPX_A:-y}" "${CPX_A+z}" "${CPX_A:+w}"; CPX_A=; echo "${CPX_A-x}" "${CPX_A:-y}" "${CPX_A+z}" "${CPX_A:+w}"',b'x y  \n y z \n')
    def test_parameter_lazy(self):
        self.same('CPX_A=yes; echo "${CPX_A:-$(echo bad > marker)}"',b'yes\n')
        self.assertFalse((self.cwd/'marker').exists())
    def test_parameter_nested(self):
        self.same('unset CPX_A; unset CPX_B; echo "${CPX_A:-${CPX_B:-hello}}"',b'hello\n')
    def test_parameter_default_space(self): self.same('unset CPX_A; echo "${CPX_A:-hello world}"',b'hello world\n')
    def test_parameter_static_validation(self):
        p=self.runline('echo bad > marker; echo "${CPX_A:-$(source nope)}"')
        self.assertEqual(p.returncode,2,p.stderr);self.assertFalse((self.cwd/'marker').exists())
    def test_parameter_length(self):
        p=self.runline("CPX_A='中文abc'; echo ${#CPX_A}")
        self.assertEqual(p.stdout,b'5\n')
    def test_arithmetic(self): self.same('CPX_A=7; echo $((CPX_A*2+3)) $((-7/3)) $((-7%3)) $((2*(3+4)))',b'17 -2 -1 14\n')
    def test_arithmetic_checked(self):
        for expression in ['1/0','9223372036854775807+1']:
            self.assertEqual(self.runline('echo $(('+expression+'))').returncode,2)
    def test_arithmetic_variable_no_eval(self):
        self.same("CPX_A='1+2'; echo $((CPX_A))",b'',2)
    def test_arithmetic_short_circuit(self): self.same('false && echo $((1/0)); true',b'')
    def test_group_scope(self): self.same('CPX_A=outer; (CPX_A=inner; echo $CPX_A); echo $CPX_A',b'inner\nouter\n')
    def test_group_redirect_pipe(self): self.same('(echo b; echo a) | sort > out; cat out',b'a\nb\n')
    def test_group_capture(self): self.same('echo "$( (echo nested; (echo more)) )"',b'nested\nmore\n')
    def test_group_cd_isolated(self):
        (self.cwd/'child').mkdir()
        self.same('(cd child; pwd); pwd',(str(self.cwd/'child')+'\n'+str(self.cwd)+'\n').encode())
    def test_here_string_no_expansion_lists(self): self.same('cat <<< {a,b}',b'{a,b}\n')
    def test_here_string_empty(self): self.same('unset CPX_A; cat <<< $CPX_A',b'\n')
    def test_literal_dollar(self): self.same('echo cost$ $',b'cost$ $\n')
    def test_builtin_case(self): self.same('ECHO hello; TRUE',b'hello\n')
    def test_noop(self):
        p=self.runline(': > empty')
        self.assertEqual(p.returncode,0,p.stderr);self.assertEqual((self.cwd/'empty').read_bytes(),b'')
    def test_command_lookup(self):
        p=self.runline('command -v echo cat clink_nonexistent')
        self.assertEqual(p.returncode,1,p.stderr)
        self.assertEqual(p.stdout.decode().splitlines(),['echo',FIXTURE.as_posix()])
    def aliasline(self,line,aliases,old=False):
        path=self.cwd/'aliases'
        path.write_text(''.join(k.encode().hex()+'\t'+v.encode().hex()+'\n' for k,v in aliases.items()))
        args=[sys.executable,str(OLD)] if old else native_args()
        return subprocess.run(args+['--aliases',str(path),'-c',line],cwd=self.cwd,capture_output=True,timeout=20)
    def test_alias_arguments(self):
        aliases={'say':'echo $2 $1 $*'}
        line='say "a b" \'中文 & %\''
        p=self.aliasline(line,aliases)
        self.assertEqual(p.stdout,'中文 & % a b a b 中文 & %\n'.encode())
        if OLD: self.assertEqual(p.stdout,self.aliasline(line,aliases,True).stdout)
    def test_alias_redirection(self):
        p=self.aliasline('say hello',{'say':'echo $* $G out'})
        self.assertEqual(p.returncode,0,p.stderr);self.assertEqual((self.cwd/'out').read_bytes(),b'hello\n')
    def test_alias_nested_capture(self):
        p=self.aliasline('echo "$(say hello)"',{'say':'echo $*'})
        self.assertEqual(p.stdout,b'hello\n')
    def test_alias_compound_static_rejection(self):
        p=self.aliasline('echo bad > marker; say',{'say':'echo a $T echo b'})
        self.assertEqual(p.returncode,2,p.stderr);self.assertFalse((self.cwd/'marker').exists())
    def test_alias_recursion(self):
        p=self.aliasline('a',{'a':'b','b':'a'})
        self.assertEqual(p.returncode,2,p.stderr)
    def test_sync_status_on_parse_failure(self):
        path=self.cwd/'sync.cmd'
        p=subprocess.run(native_args()+['--sync',str(path),'-c',"echo '"],cwd=self.cwd,capture_output=True,timeout=10)
        self.assertEqual(p.returncode,2)
        self.assertIn(b'exit /b 2',path.read_bytes())
    def test_state_atomic_no_overwrite(self):
        path=self.cwd/'state';path.write_text('preserve')
        p=subprocess.run(native_args()+['--state',str(path),'-c','true'],cwd=self.cwd,capture_output=True,timeout=10)
        self.assertEqual(p.returncode,2);self.assertEqual(path.read_text(),'preserve')

if __name__=='__main__':
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument('--python-reference', type=Path)
    options, remaining = parser.parse_known_args()
    OLD = options.python_reference
    if OLD and not OLD.is_file():
        parser.error('--python-reference must name an existing helper')
    prepare_fixture()
    unittest.main(argv=[sys.argv[0]] + remaining, verbosity=2)
