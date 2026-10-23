import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import generic.jar.ResourceFile;
import ghidra.GhidraApplicationLayout;
import ghidra.app.script.GhidraState;
import ghidra.app.services.ProgramManager;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;
import ghidra.framework.model.DomainFile;
import ghidra.framework.model.DomainFolder;
import ghidra.framework.model.Project;
import ghidra.framework.model.ProjectData;
import ghidra.program.database.ProgramDB;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.lang.CompilerSpecID;
import ghidra.program.model.lang.LanguageID;
import ghidra.program.model.listing.Program;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.util.DefaultLanguageService;
import ghidra.util.exception.CancelledException;
import ghidra.util.task.TaskMonitor;

/** Exercises startup resolution and real import transactions without opening the game project. */
public class AzReflectionStartupTest {
    private enum Case { ATTACHED, ACTIVE_MANAGER, OPEN_TARGET, PROJECT_TARGET, AMBIGUOUS, PICKER, CANCEL, NO_MANAGER }

    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration());
        for (Case scenario : Case.values()) check(scenario);
        System.out.println("PASS: attached and manager-active programs, open target, nested project target, " +
            "ambiguity, picker ownership, cancellation, missing tool, target isolation and import transactions");
    }

    private static void check(Case scenario) throws Exception {
        var language = DefaultLanguageService.getLanguageService().getLanguage(new LanguageID("x86:LE:64:default"));
        Object owner = new Object();
        ProgramDB target = new ProgramDB("NewWorld 3-26", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), owner);
        ProgramDB other = new ProgramDB(scenario == Case.AMBIGUOUS ? "NewWorld_3-26" : "NewWorld 2-1", language,
            language.getCompilerSpecByID(new CompilerSpecID("windows")), owner);
        Path resources = Files.createTempDirectory("az-startup-test-");
        try {
            int setup = target.startTransaction("startup fixture");
            try {
                target.setImageBase(target.getAddressFactory().getDefaultAddressSpace().getAddress(0x140000000L), false);
                var address = target.getImageBase().add(0x100);
                target.getMemory().createInitializedBlock("fixture", address, 16, (byte) 0xc3, TaskMonitor.DUMMY, false).setExecute(true);
                target.getFunctionManager().createFunction("FUN_fixture", address, new AddressSet(address), SourceType.DEFAULT);
            }
            finally { target.endTransaction(setup, true); }
            Files.writeString(resources.resolve("behavior-context.json"), """
                {"ebuses":[{"name":"MB::StartupBus","m_events":[{"name":"Run",
                 "m_broadcast":{"m_functionPtr":"NewWorld+0x100","virtualDispatchSlotOffset":"0x8"}}]}]}
                """);
            StringWriter console = new StringWriter();
            StartupScript script = new StartupScript(scenario, target, other, console);
            DomainFolder child = folder(scenario == Case.PROJECT_TARGET ? new DomainFile[]{target.getDomainFile()} :
                new DomainFile[]{other.getDomainFile()}, new DomainFolder[0]);
            DomainFolder root = folder(new DomainFile[0], new DomainFolder[]{child});
            ProjectData data = proxy(ProjectData.class, (method, values) -> switch (method) {
                case "getRootFolder" -> root;
                default -> throw new UnsupportedOperationException(method);
            });
            Project project = proxy(Project.class, (method, values) -> switch (method) {
                case "getProjectData" -> data;
                case "getProjectLocator" -> null;
                case "isClosed" -> false;
                default -> throw new UnsupportedOperationException(method);
            });
            script.set(new GhidraState(null, project, scenario == Case.ATTACHED ? target : null, null, null, null),
                TaskMonitor.DUMMY, new PrintWriter(console));
            script.setSourceFile(new ResourceFile("AzReflectionRenamer.java"));
            script.setScriptArgs(new String[]{"--resources", resources.toString()});
            int before = target.getDataTypeManager().getDataTypeCount(true);
            int otherBefore = other.getDataTypeManager().getDataTypeCount(true);
            script.start();
            try { script.run(); }
            finally { script.end(true); }
            boolean stopped = scenario == Case.NO_MANAGER || scenario == Case.CANCEL;
            if (stopped) {
                if (target.getDataTypeManager().getDataTypeCount(true) != before ||
                    Files.exists(resources.resolve("reflection-report.json"))) {
                    throw new AssertionError("Unattached startup changed the program or wrote a report: " + scenario);
                }
                if (!console.toString().contains("CodeBrowser")) throw new AssertionError("Startup guidance missing");
            }
            else {
                if (script.getState().getCurrentProgram() != target ||
                    target.getDataTypeManager().getDataTypeCount(true) <= before ||
                    !Files.isRegularFile(resources.resolve("reflection-report.json"))) {
                    throw new AssertionError("Startup did not bind and import into the target: " + scenario);
                }
                if (script.activations != (scenario == Case.ATTACHED ? 0 : 1)) {
                    throw new AssertionError("Program activation flow bypassed: " + scenario);
                }
            }
            if (other.getDataTypeManager().getDataTypeCount(true) != otherBefore) {
                throw new AssertionError("Startup edited another program: " + scenario);
            }
            if (script.choices != (List.of(Case.AMBIGUOUS, Case.PICKER, Case.CANCEL).contains(scenario) ? 1 : 0)) {
                throw new AssertionError("Unexpected program picker: " + scenario);
            }
            if (target.getConsumerList().contains(script)) throw new AssertionError("Picker consumer leaked");
            if (script.projectOpens != (scenario == Case.PROJECT_TARGET ? 1 : 0)) {
                throw new AssertionError("Project target not opened exactly once: " + scenario);
            }
        }
        finally {
            target.release(owner);
            other.release(owner);
            try (var paths = Files.walk(resources)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static class StartupScript extends AzReflectionRenamer {
        final Case scenario;
        final Program target;
        final Program other;
        final StringWriter console;
        int activations;
        int choices;
        int projectOpens;

        StartupScript(Case scenario, Program target, Program other, StringWriter console) {
            this.scenario = scenario; this.target = target; this.other = other; this.console = console;
        }

        @Override protected ProgramManager startupProgramManager() {
            if (scenario == Case.NO_MANAGER) return null;
            return proxy(ProgramManager.class, (method, values) -> switch (method) {
                case "getCurrentProgram" -> scenario == Case.ACTIVE_MANAGER ? target : null;
                case "getAllOpenPrograms" -> switch (scenario) {
                    case OPEN_TARGET, AMBIGUOUS -> new Program[]{other, target};
                    default -> new Program[]{other};
                };
                case "openProgram" -> {
                    if (values[0] != target.getDomainFile()) throw new AssertionError("Wrong project file opened");
                    projectOpens++;
                    yield target;
                }
                default -> throw new UnsupportedOperationException(method);
            });
        }

        @Override protected Program chooseStartupProgram() throws CancelledException {
            choices++;
            if (scenario == Case.CANCEL) return null;
            target.addConsumer(this);
            return target;
        }

        @Override public void openProgram(Program program) {
            // Replace the GUI adapter; use real Ghidra state rebinding and owned transactions.
            activations++;
            end(true);
            set(new GhidraState(null, state.getProject(), program, null, null, null),
                TaskMonitor.DUMMY, new PrintWriter(console));
            start();
        }
    }

    private static DomainFolder folder(DomainFile[] files, DomainFolder[] children) {
        return proxy(DomainFolder.class, (method, values) -> switch (method) {
            case "getFiles" -> files;
            case "getFolders" -> children;
            default -> throw new UnsupportedOperationException(method);
        });
    }

    private interface Handler { Object call(String method, Object[] values) throws Throwable; }

    private static <T> T proxy(Class<T> type, Handler handler) {
        return type.cast(Proxy.newProxyInstance(type.getClassLoader(), new Class<?>[]{type},
            (object, method, values) -> handler.call(method.getName(), values)));
    }
}
