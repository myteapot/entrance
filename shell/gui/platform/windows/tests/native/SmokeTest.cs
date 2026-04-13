using System;
using System.IO;
using FlaUI.Core;
using FlaUI.UIA3;
using NUnit.Framework;

[TestFixture]
public class SmokeTest
{
    [Test]
    public void MainWindowAppears()
    {
        var exePath = Environment.GetEnvironmentVariable("ENTRANCE_EXE_PATH")
            ?? Path.Combine("..", "..", "..", "..", "..", "..", "target", "release", "entrance-gui.exe");

        using var app = Application.Launch(exePath);
        using var automation = new UIA3Automation();
        var window = app.GetMainWindow(automation, TimeSpan.FromSeconds(30));

        Assert.That(window, Is.Not.Null, "Main window should appear within 30 seconds");
        Assert.That(window!.Title, Does.Contain("Entrance"));

        app.Close();
    }
}
