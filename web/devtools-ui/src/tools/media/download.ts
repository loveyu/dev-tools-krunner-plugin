/** 通过临时 <a download> 触发浏览器/WebView 下载。 */
export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  try {
    const link = document.createElement('a');
    link.href = url;
    link.download = filename;
    link.click();
  } finally {
    // 给 WebView 下载处理器留出启动时间，再回收 blob URL；
    // 同步回收会让 WebKitGTK 的异步下载拿到空内容。
    window.setTimeout(() => {
      URL.revokeObjectURL(url);
    }, 10_000);
  }
}
