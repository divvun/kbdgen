// The console program of the console cases of the VM and acceptance tests
// (tsf.test.vm, tsf.test.acceptance): writes its console window's handle
// to "<file>.hwnd", reads one line with ReadConsoleW, and writes that
// line's UTF-16 units as space-separated hex to <file>.
using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;

public static class ConsoleReader {
  [DllImport("kernel32.dll")] static extern IntPtr GetStdHandle(int handle);
  [DllImport("kernel32.dll")] static extern IntPtr GetConsoleWindow();
  [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern bool ReadConsoleW(IntPtr input, char[] buffer, int length, out int read, IntPtr control);

  public static void Main(string[] args) {
    File.WriteAllText(args[0] + ".hwnd", ((long)GetConsoleWindow()).ToString());
    var buffer = new char[256];
    int read;
    ReadConsoleW(GetStdHandle(-10), buffer, buffer.Length, out read, IntPtr.Zero);
    var units = new List<string>();
    foreach (char unit in new string(buffer, 0, read).TrimEnd('\r', '\n')) units.Add(((int)unit).ToString("x4"));
    File.WriteAllText(args[0], string.Join(" ", units.ToArray()));
  }
}
