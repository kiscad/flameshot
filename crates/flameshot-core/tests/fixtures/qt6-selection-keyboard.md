# Qt keyboard selection reference

Generated with Qt 6.4.2 from the operations in the Linux baseline
`f4f2e4e8`, `src/widgets/capture/selectionwidget.cpp` (keyboard slots and
`setGeometryByKeyboard`). Source SHA-256:
`5fe9b8515b32678e0e030319a518f462e25405a355cedf6fc9b3dbc4715ee336`.

Each row contains input x/y/width/height, dx/dy, resize/symmetric flags,
image width/height, then output x/y/width/height. The 156 cases cover all four
edges, full-image selection, one/two/three-pixel dimensions and every keyboard
direction. Empty intersections retain the baseline's 1×1 rectangle at the origin.
These are keyboard semantics; mouse dragging retains its dimensions at boundaries.

Reproduction uses the following standalone program, compiled against Qt Core
headers and library (no application or display server is needed). Redirect stdout
to `qt6-selection-keyboard.tsv`; stderr records the Qt runtime version. Production
and routine Rust tests do not compile or link Qt.

```cpp
#include <QRect>
#include <QtGlobal>
#include <iostream>
int main() {
  std::cerr << qVersion() << '\n';
  const QRect bounds(0,0,100,100);
  for (const auto r : {QRect(20,30,40,50),QRect(0,20,10,10),QRect(90,20,10,10),QRect(20,0,10,10),QRect(20,90,10,10),QRect(20,20,1,1),QRect(20,20,2,2),QRect(20,20,3,3),QRect(0,20,1,4),QRect(99,20,1,4),QRect(20,0,4,1),QRect(20,99,4,1),QRect(0,0,100,100)}) {
    for (int mode=0;mode<3;++mode) for (auto step : {QPoint(-1,0),QPoint(1,0),QPoint(0,-1),QPoint(0,1)}) {
      const int dx=step.x(),dy=mode==2?-step.y():step.y();
      QRect next = mode==0 ? r.adjusted(dx,dy,dx,dy) : mode==1 ? r.adjusted(0,0,dx,dy) : r.adjusted(-dx,-dy,dx,dy);
      next=next.intersected(bounds);
      if(next.width()<=0) next.setWidth(1);
      if(next.height()<=0) next.setHeight(1);
      std::cout<<r.x()<<'\t'<<r.y()<<'\t'<<r.width()<<'\t'<<r.height()<<'\t'<<step.x()<<'\t'<<step.y()<<'\t'<<(mode!=0)<<'\t'<<(mode==2)<<"\t100\t100\t"<<next.x()<<'\t'<<next.y()<<'\t'<<next.width()<<'\t'<<next.height()<<'\n';
    }
  }
}
```
