#pragma once

#include "cfapi.h"

class APIFactoryWrap
{
    // protected:
public:
    cfapi::APIFactory *ptr;
    cfapi::Session *session;
    cfapi::UserInfo *primaryUser;

    APIFactoryWrap(const std::string &appName, const std::string &appVersion,
                   bool debug, const std::string &logFileName, std::string usage,
                   const std::string &username, const std::string &password,
                   const cfapi::UserEventHandler &userHandler,
                   const cfapi::SessionEventHandler &sessionHandler);
    ~APIFactoryWrap();
};