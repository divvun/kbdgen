// The VM test's driver (tsf.test.vm): registers the test profiles, and
// types scan codes with SendInput into a Win32 EDIT, a RichEdit or a WPF
// TextBox with a text service profile active in this process, printing
// what each control then holds.
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

  // "2a+29" presses 2a, then 29, and releases them in reverse order.
  static void Chord(string chord) {
    var codes = new List<int>();
    foreach (var part in chord.Split('+')) codes.Add(Convert.ToInt32(part, 16));
    var inputs = new List<Input>();
    foreach (var code in codes) inputs.Add(Key(code, false));
    for (int i = codes.Count - 1; i >= 0; i--) inputs.Add(Key(codes[i], true));
    SendInput((uint)inputs.Count, inputs.ToArray(), Marshal.SizeOf(typeof(Input)));
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
