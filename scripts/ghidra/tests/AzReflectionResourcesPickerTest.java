import java.io.File;
import java.lang.reflect.Method;

import docking.widgets.filechooser.GhidraFileChooser;
import docking.widgets.filechooser.GhidraFileChooserMode;
import ghidra.GhidraApplicationLayout;
import ghidra.framework.Application;
import ghidra.framework.ApplicationConfiguration;

/** Exercises Ghidra's actual empty-selection approval rule without showing a dialog. */
public class AzReflectionResourcesPickerTest {
    public static void main(String[] args) throws Exception {
        Application.initializeApplication(new GhidraApplicationLayout(new File(args[0])), new ApplicationConfiguration() {
            @Override public boolean isHeadless() { return false; }
        });
        Method directorySelection = GhidraFileChooser.class.getDeclaredMethod("getUnvalidatedDirectory");
        directorySelection.setAccessible(true);
        GhidraFileChooser chooser;
        if (args.length > 1 && args[1].equals("legacy")) {
            chooser = new GhidraFileChooser(null);
            chooser.setFileSelectionMode(GhidraFileChooserMode.FILES_AND_DIRECTORIES);
        }
        else {
            Method create = AzReflectionRenamer.class.getDeclaredMethod("createReflectionResourcesChooser");
            create.setAccessible(true);
            chooser = (GhidraFileChooser) create.invoke(new AzReflectionRenamer());
        }
        try {
            File selected = (File) directorySelection.invoke(chooser);
            if (selected == null || !selected.equals(chooser.getCurrentDirectory())) {
                throw new AssertionError("Import reflection does not select the current folder when no child is selected");
            }
            System.out.println("PASS: actual Ghidra chooser approval uses the current directory without selecting its parent or a child");
        }
        finally { chooser.dispose(); }
    }
}
