describe("Folder Search アプリ", () => {
  it("アプリウィンドウが表示される", async () => {
    const title = await browser.getTitle();
    expect(title).toBe("Folder Search");
  });

  it("サイドバーにアプリ名が表示される", async () => {
    const heading = await $("h2");
    const text = await heading.getText();
    expect(text).toBe("Folder Search");
  });

  it("メインパネルが存在する", async () => {
    const main = await $("main");
    expect(await main.isExisting()).toBe(true);
  });
});
