// The VM test's driver (tsf.test.vm): calls the registration exports,
// registers the test profiles, and types scan codes with SendInput into a
// Win32 EDIT, a RichEdit or a WPF TextBox with a text service profile
// active in this process, or into a console, printing what each control
// then holds.
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Forms;

[ComImport, Guid("71c6e74c-0f28-11d8-a82a-00065b84435c"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IProfileMgr {
  [PreserveSig] int ActivateProfile(uint type, ushort langid, ref Guid clsid, ref Guid profile, IntPtr hkl, uint flags);
  [PreserveSig] int DeactivateProfile(uint type, ushort langid, ref Guid clsid, ref Guid profile, IntPtr hkl, uint flags);
  [PreserveSig] int GetProfile(uint type, ushort langid, ref Guid clsid, ref Guid profile, IntPtr hkl, IntPtr result);
  [PreserveSig] int EnumProfiles(ushort langid, out IntPtr profiles);
  [PreserveSig] int ReleaseInputProcessor(ref Guid clsid, uint flags);
  [PreserveSig] int RegisterProfile(ref Guid clsid, ushort langid, ref Guid profile,
    [MarshalAs(UnmanagedType.LPWStr)] string desc, uint descLen,
    [MarshalAs(UnmanagedType.LPWStr)] string icon, uint iconLen, uint iconIndex,
    IntPtr hklSubstitute, uint preferredLayout, int enabledByDefault, uint flags);
  [PreserveSig] int UnregisterProfile(ref Guid clsid, ushort langid, ref Guid profile, uint flags);
  [PreserveSig] int GetActiveProfile(ref Guid category, out ProfileInfo profile);
}

[StructLayout(LayoutKind.Sequential)]
struct ProfileInfo {
  public uint ProfileType;
  public ushort LangId;
  public Guid Clsid;
  public Guid Profile;
  public Guid Category;
  public IntPtr HklSubstitute;
  public uint Caps;
  public IntPtr Hkl;
  public uint Flags;
}

public static class TsfDriver {
  [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr process);
  [DllImport("user32.dll")] static extern bool AttachThreadInput(uint from, uint to, bool attach);
  [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern IntPtr LoadKeyboardLayout(string klid, uint flags);
  [DllImport("user32.dll")] static extern bool UnloadKeyboardLayout(IntPtr hkl);
  [DllImport("user32.dll")] static extern IntPtr ActivateKeyboardLayout(IntPtr hkl, uint flags);
  [DllImport("user32.dll")] static extern uint SendInput(uint n, Input[] inputs, int size);

  [StructLayout(LayoutKind.Sequential)] struct KeybdInput { public ushort Vk; public ushort Scan; public uint Flags; public uint Time; public IntPtr Extra; }
  [StructLayout(LayoutKind.Sequential)] struct MouseInput { public int Dx, Dy; public uint Data, Flags, Time; public IntPtr Extra; }
  [StructLayout(LayoutKind.Explicit)] struct InputUnion { [FieldOffset(0)] public KeybdInput Key; [FieldOffset(0)] public MouseInput Mouse; }
  [StructLayout(LayoutKind.Sequential)] struct Input { public uint Type; public InputUnion U; }

  static readonly Guid ProfilesClsid = new Guid("33C53A50-F456-4884-B049-85FD643ECFED");
  static readonly Guid KeyboardCategory = new Guid("34745c63-b2f0-4784-8b67-5e12c8701a31");
  const uint ForProcessDontCareLanguage = 0x10000004;

  static bool wpf;

  static IProfileMgr Profiles() {
    return (IProfileMgr)Activator.CreateInstance(Type.GetTypeFromCLSID(ProfilesClsid));
  }

  public static int Register(string clsid, ushort langid, string profile, string name, string icon) {
    var c = new Guid(clsid); var p = new Guid(profile);
    return Profiles().RegisterProfile(ref c, langid, ref p, name, (uint)name.Length, icon, (uint)icon.Length, 0, IntPtr.Zero, 0, 1, 0);
  }

  public static int Unregister(string clsid, ushort langid, string profile) {
    var c = new Guid(clsid); var p = new Guid(profile);
    return Profiles().UnregisterProfile(ref c, langid, ref p, 0);
  }

  [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern IntPtr LoadLibraryW(string path);
  [DllImport("kernel32.dll", CharSet = CharSet.Ansi)] static extern IntPtr GetProcAddress(IntPtr module, string name);
  [DllImport("kernel32.dll")] static extern bool FreeLibrary(IntPtr module);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int Export();

  // Calls DllRegisterServer or DllUnregisterServer of the DLL at path in
  // this process, for the HRESULT that regsvr32 /s hides, and unloads it
  // again so that the file can be deleted.
  public static int CallExport(string path, string name) {
    var module = LoadLibraryW(path);
    if (module == IntPtr.Zero) { return Marshal.GetHRForLastWin32Error(); }
    try {
      var export = GetProcAddress(module, name);
      if (export == IntPtr.Zero) { return unchecked((int)0x8007007F); }
      return ((Export)Marshal.GetDelegateForFunctionPointer(export, typeof(Export)))();
    } finally {
      FreeLibrary(module);
    }
  }

  static void Pump() {
    if (wpf) {
      var frame = new System.Windows.Threading.DispatcherFrame();
      var timer = new System.Windows.Threading.DispatcherTimer { Interval = TimeSpan.FromMilliseconds(120) };
      timer.Tick += (o, e) => { timer.Stop(); frame.Continue = false; };
      timer.Start();
      System.Windows.Threading.Dispatcher.PushFrame(frame);
      return;
    }
    for (int i = 0; i < 8; i++) { Application.DoEvents(); System.Threading.Thread.Sleep(15); }
  }

  // Takes the foreground from whichever window has it, so that the input
  // SendInput makes reaches this window and nowhere else.
  static void Foreground(IntPtr window) {
    uint other = GetWindowThreadProcessId(GetForegroundWindow(), IntPtr.Zero);
    uint self = GetCurrentThreadId();
    bool attached = other != self && AttachThreadInput(self, other, true);
    BringWindowToTop(window);
    SetForegroundWindow(window);
    if (attached) AttachThreadInput(self, other, false);
  }

  static Input Key(int code, bool up) {
    var input = new Input { Type = 1 };
    input.U.Key.Scan = (ushort)(code & 0xff);
    input.U.Key.Flags = 0x8u | (up ? 0x2u : 0u) | ((code & 0xff00) == 0xe000 ? 0x1u : 0u);
    return input;
  }

  // "2a+29" presses 2a, then 29, and releases them in reverse order. Each
  // press is its own SendInput a few ticks after the last, so that a Left
  // Ctrl held before Right Alt has an earlier message time than Right Alt,
  // as a user's does, and is not taken for the one AltGr synthesises.
  static void Chord(string chord) {
    var codes = new List<int>();
    foreach (var part in chord.Split('+')) codes.Add(Convert.ToInt32(part, 16));
    var size = Marshal.SizeOf(typeof(Input));
    for (int i = 0; i < codes.Count; i++) {
      if (i > 0) System.Threading.Thread.Sleep(40);
      SendInput(1, new[] { Key(codes[i], false) }, size);
    }
    var inputs = new List<Input>();
    for (int i = codes.Count - 1; i >= 0; i--) inputs.Add(Key(codes[i], true));
    SendInput((uint)inputs.Count, inputs.ToArray(), size);
    Pump();
  }

  static string Hex(string text) {
    var units = new List<string>();
    foreach (char c in text) units.Add(((int)c).ToString("x4"));
    return string.Join(" ", units.ToArray());
  }

  static string Unhex(string units) {
    var text = new StringBuilder();
    foreach (var unit in units.Split(',')) text.Append((char)Convert.ToInt32(unit, 16));
    return text.ToString();
  }

  // Types every case into a fresh console window (conhost) running
  // `reader`, a console program that writes what ReadConsoleW returns, as
  // UTF-16 hex, to the file it is given, and that file's name with
  // ".hwnd" appended holding its console window. `toggle` is the chord that
  // switches the console's input to the text service and back, as a user
  // does with Win+Space: a console started from here takes the session's
  // input, not the profile this process activates. Enter ends each case.
  // Cases that put text without typing ("set:") are skipped.
  public static string TypeConsole(string reader, string toggle, string[] cases) {
    var log = new StringBuilder();
    var output = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "kbd-tsf-console.txt");
    foreach (var test in cases) {
      var parts = test.Split('=');
      if (parts[1].Contains("set:")) continue;
      System.IO.File.Delete(output);
      System.IO.File.Delete(output + ".hwnd");
      var console = System.Diagnostics.Process.Start("conhost.exe", "\"" + reader + "\" \"" + output + "\"");
      for (int i = 0; i < 100 && !System.IO.File.Exists(output + ".hwnd"); i++) System.Threading.Thread.Sleep(100);
      System.Threading.Thread.Sleep(300);
      var window = IntPtr.Zero;
      if (System.IO.File.Exists(output + ".hwnd")) window = new IntPtr(long.Parse(System.IO.File.ReadAllText(output + ".hwnd")));
      Foreground(window);
      System.Threading.Thread.Sleep(300);
      bool foreground = GetForegroundWindow() == window;
      Chord(toggle);
      foreach (var chord in parts[1].Split(' ')) Chord(chord);
      Chord(toggle);
      Chord("1c");
      for (int i = 0; i < 50 && !System.IO.File.Exists(output); i++) System.Threading.Thread.Sleep(100);
      var text = System.IO.File.Exists(output) ? System.IO.File.ReadAllText(output) : "none";
      if (!console.HasExited) console.Kill();
      log.AppendFormat("RESULT console 64 {0} => {1}\n", parts[0], foreground ? text : "background");
    }
    return log.ToString();
  }

  // Types every case ("name=chord chord ...") into a fresh control of kind
  // `kind` (edit, rich or wpf). A chord "set:d83d,de00" instead puts those
  // UTF-16 units into the control without typing, caret at the end. With
  // `klid` set, the layout DLL alone is active instead of the text service.
  public static string TypeCases(string kind, string clsid, string profile, ushort langid, string klid, string[] cases) {
    wpf = kind == "wpf";
    var log = new StringBuilder();
    IntPtr handle = IntPtr.Zero;
    Func<string> read;
    Action<string> put;
    Action clear;
    Action focus;
    if (wpf) {
      var window = new System.Windows.Window { Title = "kbd-tsf vm", Width = 400, Height = 140, Topmost = true };
      var box = new System.Windows.Controls.TextBox();
      window.Content = box;
      window.Show();
      read = () => box.Text;
      put = text => { box.Text = text; box.CaretIndex = text.Length; };
      clear = () => box.Clear();
      focus = () => {
        handle = new System.Windows.Interop.WindowInteropHelper(window).Handle;
        Foreground(handle);
        window.Activate();
        box.Focus();
        System.Windows.Input.Keyboard.Focus(box);
      };
    } else {
      var form = new Form { Text = "kbd-tsf vm", Width = 400, Height = 140, TopMost = true };
      TextBoxBase box = kind == "rich" ? (TextBoxBase)new RichTextBox { Dock = DockStyle.Fill } : new TextBox { Multiline = true, Dock = DockStyle.Fill };
      form.Controls.Add(box);
      form.Show();
      read = () => box.Text;
      put = text => { box.Text = text; box.SelectionStart = box.TextLength; };
      clear = () => box.Clear();
      focus = () => { handle = form.Handle; Foreground(handle); form.Activate(); box.Focus(); };
    }
    Pump();
    IntPtr hkl = IntPtr.Zero;
    int hr = 0;
    var mgr = Profiles();
    var keyboards = KeyboardCategory;
    ProfileInfo previous = new ProfileInfo();
    bool restore = klid == "";
    if (restore && (mgr.GetActiveProfile(ref keyboards, out previous) != 0 || previous.Clsid == new Guid(clsid))) {
      previous = new ProfileInfo { ProfileType = 2, LangId = 0x0409, Hkl = new IntPtr(0x04090409) };
    }
    if (klid != "") {
      hkl = LoadKeyboardLayout(klid, 0);
      ActivateKeyboardLayout(hkl, 0);
    } else {
      var tip = new Guid(clsid); var keyboard = new Guid(profile);
      hr = mgr.ActivateProfile(1, langid, ref tip, ref keyboard, IntPtr.Zero, ForProcessDontCareLanguage);
    }
    focus();
    Pump();
    var category = KeyboardCategory;
    ProfileInfo active;
    int activeHr = mgr.GetActiveProfile(ref category, out active);
    bool ours = activeHr == 0 && active.Clsid == new Guid(clsid) && active.Profile == new Guid(profile);
    log.AppendFormat("SETUP {0} {1} activate=0x{2:x} ours={3} hkl=0x{4:x} foreground={5}\n", kind, IntPtr.Size * 8, hr, ours, (long)hkl, GetForegroundWindow() == handle);
    foreach (var test in cases) {
      var parts = test.Split('=');
      clear();
      Pump();
      if (GetForegroundWindow() != handle) { focus(); Pump(); }
      Chord("e04f");
      foreach (var chord in parts[1].Split(' ')) {
        if (chord.StartsWith("set:")) { put(Unhex(chord.Substring(4))); Pump(); } else Chord(chord);
      }
      Pump();
      log.AppendFormat("RESULT {0} {1} {2} => {3}\n", kind + (klid != "" ? "-dll" : ""), IntPtr.Size * 8, parts[0], Hex(read()));
    }
    if (hkl != IntPtr.Zero) UnloadKeyboardLayout(hkl);
    // Activating a profile switches the input method of the whole session,
    // so the one active before is restored.
    if (restore) {
      var c = previous.Clsid; var p = previous.Profile;
      int restored = mgr.ActivateProfile(previous.ProfileType, previous.LangId, ref c, ref p, previous.Hkl, ForProcessDontCareLanguage);
      log.AppendFormat("RESTORE {0} {1} type={2} lang={3:x4} hkl=0x{4:x} hr=0x{5:x}\n", kind, IntPtr.Size * 8, previous.ProfileType, previous.LangId, (long)previous.Hkl, restored);
    }
    return log.ToString();
  }
}
