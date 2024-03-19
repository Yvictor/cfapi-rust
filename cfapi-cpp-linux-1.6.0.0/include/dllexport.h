#ifndef DLLEXPORT_H
#define DLLEXPORT_H

#ifdef DLLEXPORTS
#define CFAPI_DLLEXPORTS __declspec(dllexport)
#else
#define CFAPI_DLLEXPORTS 
#endif

#endif