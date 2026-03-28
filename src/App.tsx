import { Sidebar } from "./components/layout/Sidebar";
import { MainPanel } from "./components/layout/MainPanel";

function App() {
  return (
    <div className="app">
      <Sidebar />
      <MainPanel>
        <p>Phase 1: 基盤構築完了</p>
      </MainPanel>
    </div>
  );
}

export default App;
