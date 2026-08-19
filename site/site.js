document.querySelectorAll("[data-copy]").forEach((btn) => {
  btn.addEventListener("click", async () => {
    const text = btn.getAttribute("data-copy") || "";
    try {
      await navigator.clipboard.writeText(text);
      btn.classList.add("done");
      const prev = btn.textContent;
      btn.textContent = "copied";
      setTimeout(() => {
        btn.classList.remove("done");
        btn.textContent = prev;
      }, 1400);
    } catch {
      btn.textContent = "failed";
    }
  });
});
