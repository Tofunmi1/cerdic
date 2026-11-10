/* The convex hull API Blender's bmesh `convex_hull` operator (bmo_hull.cc) calls. Upstream it is implemented over Bullet's btConvexHullComputer (intern/rigidbody);
 * here it is our own incremental 3D hull in cpp/blender/ops/hull3d.cc, so no Bullet is built. Same names, same meaning: a hull is a list of vertices (each remembering the
 * index of the input point it is) and a list of faces as vertex-index loops (this implementation returns triangles, wound counter-clockwise seen from outside). */
#ifndef VE_RBI_HULL_API_H
#define VE_RBI_HULL_API_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct plConvexHull__ {
  int unused;
} *plConvexHull;

plConvexHull plConvexHullCompute(float (*coords)[3], int count);
void plConvexHullDelete(plConvexHull hull);
int plConvexHullNumVertices(plConvexHull hull);
int plConvexHullNumFaces(plConvexHull hull);
void plConvexHullGetVertex(plConvexHull hull, int n, float coords[3], int *original_index);
int plConvexHullGetFaceSize(plConvexHull hull, int n);
void plConvexHullGetFaceVertices(plConvexHull hull, int n, int *r_vertices);

#ifdef __cplusplus
}
#endif

#endif
